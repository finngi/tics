//! The rule table: stable ids, metadata and the pattern each rule matches with.
//!
//! Patterns are compiled case-insensitively in verbose mode (`(?x)`: whitespace is ignored, so a
//! literal space is written `\s` or `\x20`). Case-sensitive parts use `(?-i:…)`. A named group
//! `m`, when present, is the span reported instead of the whole match.

use serde::Serialize;

const SIGNS: &str = "https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing";
const WORDS: &str = "https://en.wikipedia.org/wiki/Wikipedia:Manual_of_Style/Words_to_watch";
const BUZZ: &str = "https://en.wikipedia.org/wiki/Wikipedia:Marketing_buzzspeak";
const V0_1_0: &str = "0.1.0";
const V0_2_0: &str = "0.2.0";

/// Section labels that chat models put on their own line. From the calibration corpus
/// (DESIGN.md, "Calibration"): in about half of one model family's explanations, never in the
/// human baseline.
macro_rules! stock_label {
    () => {
        r"(?:the\x20)?(?:
            why\x20(?:it|this|that)\x20(?:matters|works)
          | how\x20(?:it|this)\x20works
          | what\x20(?:it|this)\x20means
          | key\x20(?:takeaways?|points|ideas)
          | the\x20basics
          | (?:short|long|quick)\x20answer
          | short\x20version
          | tl;?\x20?dr
          | in\x20short
          | bottom\x20line
          | in\x20a\x20nutshell
        )\b"
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity {
    Info,
    Warn,
}

impl Severity {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Category {
    Contrast,
    Rhetoric,
    Vocabulary,
    Chatbot,
    Formatting,
}

impl Category {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Contrast => "contrast",
            Self::Rhetoric => "rhetoric",
            Self::Vocabulary => "vocabulary",
            Self::Chatbot => "chatbot",
            Self::Formatting => "formatting",
        }
    }
}

/// How a rule finds hits. Each variant says which text it reads.
#[derive(Debug)]
pub(crate) enum Detector {
    /// `find` runs on each sentence of prose (`\A` = sentence start). A hit is dropped when
    /// `reject` matches the sentence from the hit onwards, or `check` returns false for it.
    Text {
        find: &'static str,
        reject: Option<&'static str>,
        check: Option<fn(&str) -> bool>,
    },
    /// Two adjacent sentences in one block: `first` then `second`, both anchored at the
    /// sentence start. With `same_subject`, both must capture the same `subj` (ignoring case).
    Pair {
        first: &'static str,
        first_reject: Option<&'static str>,
        second: &'static str,
        same_subject: bool,
    },
    /// Runs on the first source line of each Markdown list item, from its marker.
    ListItem(&'static str),
    /// Runs on each heading's prose text.
    Heading(fn(&str) -> bool),
    /// Runs on the whole source with only code and front matter removed, so it sees URLs.
    Raw(&'static str),
}

#[derive(Debug)]
pub(crate) struct Rule {
    pub(crate) id: &'static str,
    pub(crate) category: Category,
    pub(crate) severity: Severity,
    pub(crate) since: &'static str,
    pub(crate) source: &'static str,
    pub(crate) description: &'static str,
    pub(crate) detector: Detector,
}

pub(crate) static RULES: &[Rule] = &[
    // Contrast / negation.
    Rule {
        id: "negative-parallelism",
        category: Category::Contrast,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "negative parallelism (`not only X but also Y`, `isn't just X`)",
        detector: Detector::Text {
            find: r"\bnot\s+(?:only|just|merely)\s[^.!?;]{1,80}?\bbut\b(?:\s+also\b)?
                  | \b(?:is|are|was|were|does)(?:n['’]t|\s+not)\s+(?:just|merely)\s+\w
                  | \b(?:it|this|that|they)['’](?:s|re)\s+not\s+(?:just|merely)\s+\w",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "not-x-but-y",
        category: Category::Contrast,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "contrast inside one sentence (`not X, but Y`, `isn't X; it's Y`)",
        detector: Detector::Text {
            find: r"\b(?:not|(?:is|are|was|were|does)n['’]t)\s+[^.!?;:,—–\n]{1,50}?\s*,\s*but\s+\w
                  | \b(?:not|(?:is|are|was|were)n['’]t)\s+[^.!?;:,—–\n]{1,50}?\s*(?:[;,:]|[—–])\s*
                    (?:but\s+)?(?:it|this|that|they)(?:['’]s|['’]re|\s+is|\s+are)\s+
                    (?:a|an|the|about|just|simply|really|actually|more|what|how|why|where|who)\b",
            reject: Some(
                r"\A(?:not|\w+n['’]t)\s+(?:only|just|merely)\b
                  | \A[^.!?]{0,80}?,\s*but\s+(?:you|we|they|i|he|she|there|one)\b
                  | \A[^.!?]{0,80}?,\s*but\s+(?:it|this|that)\s+(?:can|will|does|do|may|might|should|could|works?|runs?|has|needs?)\b
                  | \A[^.!?]{0,80}?,\s*but\s+(?:it|this|that)\s+(?:is|was)\s+(?:not\s+)?\w+(?:ed|en)\b",
            ),
            check: None,
        },
    },
    Rule {
        id: "not-x-its-y-split",
        category: Category::Contrast,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "contrast split over two sentences (`It isn't X. It's Y.`)",
        detector: Detector::Pair {
            first: r#"\A["“]?(?:it|this|that|they)
                      (?:\s+(?:is|are|was|were|does|do)\s+not|\s+(?:is|are|was|were|does|do)n['’]t|['’]s\s+not|['’]re\s+not)\b"#,
            first_reject: None,
            second: r#"\A["“]?(?:it|this|that|they)(?:['’]s|['’]re|\s+is|\s+are|\s+was)\s+
                       (?:a|an|the|about|just|simply|really|actually|more|what|how|why|where|who)\b"#,
            same_subject: false,
        },
    },
    Rule {
        id: "x-not-y",
        category: Category::Contrast,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "appositive contrast (`X, not Y`)",
        detector: Detector::Text {
            find: r"\s?(?P<m>(?:,|[—–])\s*not\s+[^,.;:!?\n—–()]{1,40})",
            reject: Some(
                r"\A\s?(?:,|[—–])\s*not\s+(?:yet|necessarily|always|least|to\s+mention|much|many|even|
                  quite|sure|that|so|too|very|really|enough|if|when|unless|counting|including)\b
                  | \A\s?(?:,|[—–])\s*not\s+(?:only|just|merely)\s[^.!?;]{1,80}?\bbut\b",
            ),
            check: None,
        },
    },
    Rule {
        id: "does-x-does-not-y",
        category: Category::Contrast,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "same subject affirmed then denied (`It does X. It does not Y.`)",
        detector: Detector::Pair {
            first: r#"\A["“]?(?P<subj>(?:(?:the|this|that|a|an|our|your|its)\s+)?[\w-]+)\s+\w+"#,
            first_reject: Some(r"\bnot\b|n['’]t\b|\bnever\b|\bno\b"),
            second: r#"\A["“]?(?P<subj>(?:(?:the|this|that|a|an|our|your|its)\s+)?[\w-]+)\s+
                       (?:(?:does|do|did|is|are|was|were|can|will|could|should|would)\s+not
                       |(?:does|do|did|is|are|was|were|could|should|would|ca|wo)n['’]t|cannot|never)\b"#,
            same_subject: true,
        },
    },
    Rule {
        id: "rather-than",
        category: Category::Contrast,
        severity: Severity::Info,
        since: V0_1_0,
        source: SIGNS,
        description: "contrast framing (`rather than`, `instead of`)",
        detector: Detector::Text {
            find: r"\b(?:rather\s+than|instead\s+of)\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "not-sentence-start",
        category: Category::Contrast,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "sentence opening with a negation (`Not a…`, `Not just…`, `Not because…`)",
        detector: Detector::Text {
            find: r#"\A["“(]?not\s+(?:a|an|the|just|only|because)\b"#,
            reject: None,
            check: None,
        },
    },
    // Rhetoric and structure.
    Rule {
        id: "trailing-ing-analysis",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "trailing participle analysis (`, highlighting…`, `, underscoring…`)",
        detector: Detector::Text {
            find: r",\s*(?:highlighting|underscoring|emphasi[sz]ing|reflecting|showcasing|fostering)\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "significance-puffing",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "puffed-up significance (`pivotal role`, `is a testament`, `evolving landscape`)",
        detector: Detector::Text {
            find: r"\b(?:crucial|pivotal|vital)\s+role\b
                  | \b(?:is|was|are|were|stands\s+as|serves\s+as|remains)\s+(?:a|an)\s+(?:\w+\s+)?(?:testament|reminder)\b
                  | \bset(?:s|ting)?\s+the\s+stage\b
                  | \bevolving\s+landscape\b
                  | \bindelible\s+mark\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "importance-hedge",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: WORDS,
        description: "editorialising hedge (`it's worth noting`, `it should be noted`)",
        detector: Detector::Text {
            find: r"\b(?:it|this)(?:['’]s|\s+is)\s+(?:also\s+|especially\s+|particularly\s+)?
                    (?:important|worth|crucial|essential)\s+
                    (?:to\s+(?:note|remember|mention|highlight|emphasi[sz]e|point\s+out)|noting|remembering|mentioning)\b
                  | \bshould\s+be\s+noted\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "vague-attribution",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "vague attribution (`experts argue`, `critics say`)",
        detector: Detector::Text {
            find: r"\b(?:experts|observers|critics|analysts|commentators)\s+
                    (?:argue|say|believe|suggest|agree|claim|contend|warn)\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "summary-opener",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "summary opener (`In summary,`, `Overall,`, `Ultimately,`)",
        detector: Detector::Text {
            find: r#"\A["“(]?(?:in\s+summary|in\s+conclusion|overall|ultimately|in\s+short)\s*,"#,
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "signposting",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "chatty signposting (`Here's the thing`, `Let's dive in`)",
        detector: Detector::Text {
            find: r"\bhere['’]s\s+the\s+thing\b
                  | \blet['’]s\s+(?:dive\s+(?:in|into|deeper)|delve|unpack)\b
                  | \bin\s+this\s+(?:section|article|guide|post|tutorial|chapter),?\s+we\s+(?:will|['’]ll|are\s+going\s+to)\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "rhetorical-reveal",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "rhetorical reveal (`The result?`, `The catch?`)",
        detector: Detector::Text {
            find: r#"\A["“]?the\s+(?:result|catch|answer|kicker|twist)\?"#,
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "whether-youre",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "audience sweep (`Whether you're X or Y`)",
        detector: Detector::Text {
            find: r#"\A["“]?whether\s+you(?:['’]re|\s+are)\s[^.!?]{1,80}?\bor\b"#,
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "todays-world",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "era cliché (`In today's fast-paced world`)",
        detector: Detector::Text {
            find: r"\bin\s+today['’]s\s+(?:\w+\s+)?(?:fast-paced|ever-changing|ever-evolving|rapidly\s+(?:changing|evolving)
                    |digital|modern|interconnected|competitive|data-driven|world|age|era|landscape)\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "transition-opener",
        category: Category::Rhetoric,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "stock transition opener (`Additionally,`, `Moreover,`, `Furthermore,`)",
        detector: Detector::Text {
            find: r#"\A["“(]?(?:additionally|moreover|furthermore|notably)\s*,"#,
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "rule-of-three",
        category: Category::Rhetoric,
        severity: Severity::Info,
        since: V0_1_0,
        source: SIGNS,
        description: "list of three or more adjectives or abstract nouns (`fast, reliable, and secure`)",
        detector: Detector::Text {
            find: r"(?-i:\b[a-z][a-z-]*(?:\s+[a-z][a-z-]*)?,\s+[a-z][a-z-]*(?:\s+[a-z][a-z-]*)?
                    (?:,\s+[a-z][a-z-]*(?:\s+[a-z][a-z-]*)?)*,?\s+(?:and|or)\s+[a-z][a-z-]*(?:\s+[a-z][a-z-]*)?\b)",
            reject: None,
            check: Some(is_rule_of_three),
        },
    },
    // Vocabulary.
    Rule {
        id: "ai-vocabulary",
        category: Category::Vocabulary,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "overused AI vocabulary (`delve`, `tapestry`, `robust`, `seamless`, …)",
        detector: Detector::Text {
            find: r"\b(?:delv(?:e|es|ed|ing)|tapestr(?:y|ies)|testaments?|landscapes?|robust(?:ly|ness)?
                    |leverag(?:e|es|ed|ing)|underscor(?:ed|ing)|underscores?\s+(?:the|a|an|how|why|its|our|their|this|these|those)
                    |pivotal|vibrant(?:ly)?|showcas(?:e|es|ed|ing)|garner(?:s|ed|ing)?|intricate(?:ly)?|intricac(?:y|ies)
                    |meticulous(?:ly|ness)?|seamless(?:ly)?|holistic(?:ally)?|synerg(?:y|ies|istic)|paradigms?
                    |unlock(?:s|ed|ing)?|empower(?:s|ed|ing|ment)?|elevat(?:e|es|ed|ing)|realms?
                    |game[-\s]?chang(?:er|ers|ing)|effortless(?:ly)?)\b",
            reject: Some(
                r"\A(?:landscape\s+(?:mode|orientation|format)
                  |unlock(?:s|ed|ing)?\s+(?:the\s+|a\s+|an\s+|its\s+)?(?:mutex|lock|file|screen|device|keychain|vault|wallet|account|database|row|table)
                  |elevated\s+(?:privileges?|permissions?|rights|access|shell|prompt|mode))\b",
            ),
            check: None,
        },
    },
    Rule {
        id: "promotional",
        category: Category::Vocabulary,
        severity: Severity::Warn,
        since: V0_1_0,
        source: BUZZ,
        description: "promotional buzzwords (`boasts a`, `cutting-edge`, `best-in-class`)",
        detector: Detector::Text {
            find: r"\bboasts?\s+(?:a|an)\b|\bground[-\s]?breaking\b|\bcutting[-\s]edge\b|\bworld-class\b
                  | \bstate[-\s]of[-\s]the[-\s]art\b|\bbest[-\s]in[-\s]class\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "copula-avoidance",
        category: Category::Vocabulary,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "copula avoidance (`serves as a`, `stands as the`)",
        detector: Detector::Text {
            find: r"\b(?:serv(?:e|es|ed|ing)|stand(?:s|ing)?|stood|function(?:s|ed|ing))\s+as\s+(?:a|an|the)\b",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "canned-assurance",
        category: Category::Vocabulary,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "canned assurance (`ensure that`, `in compliance with`, `streamline`)",
        detector: Detector::Text {
            find: r"\bensur(?:e|es|ed|ing)\s+that\b|\bin\s+compliance\s+with\b|\bstreamlin(?:e|es|ed|ing)\b",
            reject: None,
            check: None,
        },
    },
    // Chatbot residue.
    Rule {
        id: "chatbot-residue",
        category: Category::Chatbot,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "chatbot residue (`Certainly!`, `I hope this helps`, `as an AI`)",
        detector: Detector::Text {
            find: r"\bcertainly!|\bof\s+course!|\bgreat\s+question\b|\bi\s+hope\s+this\s+helps\b
                  | \blet\s+me\s+know\s+if\b|\bas\s+an\s+(?-i:AI)\b",
            reject: Some(r"\Aas\s+an\s+ai-"),
            check: None,
        },
    },
    Rule {
        id: "placeholder",
        category: Category::Chatbot,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "unfilled placeholder (`[Insert …]`, `INSERT_`, `YYYY-XX-XX`)",
        detector: Detector::Text {
            find: r"\[(?:insert|describe|your)\s[^\]\n]{1,60}\]|(?-i:\bINSERT_[A-Z0-9_]*)|(?-i:\b(?:YYYY|XXXX)-XX-XX\b)",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "leaked-markup",
        category: Category::Chatbot,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "leaked chatbot markup (`contentReference[oaicite`, `utm_source=chatgpt.com`)",
        detector: Detector::Raw(
            r"contentReference\[oaicite|utm_source=chatgpt\.com|referrer=grok\.com",
        ),
    },
    // Formatting.
    Rule {
        id: "em-dash",
        category: Category::Formatting,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "em dash in prose",
        detector: Detector::Text {
            find: "—",
            reject: None,
            check: None,
        },
    },
    Rule {
        id: "bold-lead-in",
        category: Category::Formatting,
        severity: Severity::Warn,
        since: V0_1_0,
        source: SIGNS,
        description: "list item led by a bold label and colon (`- **Label:** text`)",
        detector: Detector::ListItem(
            r"\A(?:[-*+]|\d{1,9}[.)])[\x20\t]+(?:\[[\x20xX]\][\x20\t]+)?
              (?P<m>(?:\*\*|__)[^*_\n]{1,80}?(?::(?:\*\*|__)|(?:\*\*|__)[\x20\t]*:))",
        ),
    },
    Rule {
        id: "bold-label-line",
        category: Category::Formatting,
        severity: Severity::Warn,
        since: V0_2_0,
        source: SIGNS,
        description: "paragraph led by a bold label, or a bold line used as a heading (`**Why:** …`)",
        detector: Detector::Raw(
            r"(?m)^[\x20\t]{0,3}(?:
                (?:\*\*|__)[^*_\n]{1,60}?[:.]?(?:\*\*|__)[:.]?[\x20\t]*$
              | (?:\*\*|__)[^*_\n]{1,60}?[:.](?:\*\*|__)[\x20\t]
              | (?:\*\*|__)[^*_\n]{1,60}?(?:\*\*|__)[:.][\x20\t]
            )",
        ),
    },
    Rule {
        id: "stock-section-label",
        category: Category::Formatting,
        severity: Severity::Warn,
        since: V0_2_0,
        source: SIGNS,
        description: "stock section label (`Why it matters`, `How it works`, `Short answer:`, `TL;DR`)",
        detector: Detector::Raw(concat!(
            r"(?m)^[\x20\t]{0,3}(?:\#{1,6}[\x20\t]+(?:\*\*|__)?|\*\*|__)",
            stock_label!(),
            r"|(?m)^[\x20\t]{0,3}",
            stock_label!(),
            r"[\x20\t]?:",
        )),
    },
    Rule {
        id: "title-case-heading",
        category: Category::Formatting,
        severity: Severity::Info,
        since: V0_1_0,
        source: SIGNS,
        description: "heading in Title Case",
        detector: Detector::Heading(is_title_case),
    },
];

/// Looks up a rule by its id.
pub(crate) fn by_id(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.id == id)
}

/// Words that never make a list item "adjective-like" on their own.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "to", "of", "in", "on", "for", "with", "by", "is", "are", "was", "were",
    "be", "been", "it", "its", "this", "that", "these", "those", "our", "your", "their", "my",
    "we", "you", "they", "as", "at", "from", "into", "so", "very", "more", "most", "also",
];

/// Common short evaluative adjectives that carry no telltale suffix.
const ADJECTIVES: &[&str] = &[
    "fast",
    "safe",
    "secure",
    "simple",
    "clean",
    "smart",
    "lean",
    "easy",
    "quick",
    "modern",
    "sleek",
    "bold",
    "rich",
    "clear",
    "open",
    "fun",
    "free",
    "light",
    "lightweight",
    "elegant",
    "efficient",
    "robust",
    "fresh",
    "friendly",
];

const EVALUATIVE_SUFFIXES: &[&str] = &["ive", "ful", "less", "ous", "able", "ible", "ity", "ness"];

/// The rule-of-three heuristic: three or more list items, at least two of which end in an
/// evaluative adjective or abstract noun. Technical lists (tools, platforms, nouns like
/// `path`, `mode`) rarely qualify, which keeps false positives low.
pub(crate) fn is_rule_of_three(list: &str) -> bool {
    let mut items: Vec<Vec<&str>> = vec![Vec::new()];
    for piece in list.split(',') {
        for word in piece.split_whitespace() {
            if word == "and" || word == "or" {
                items.push(Vec::new());
            } else if let Some(item) = items.last_mut() {
                item.push(word);
            }
        }
        items.push(Vec::new());
    }
    items.retain(|item| !item.is_empty());
    let mut heads = Vec::with_capacity(items.len());
    for item in &items {
        match item.iter().rev().find(|word| !STOPWORDS.contains(word)) {
            Some(head) => heads.push(*head),
            None => return false,
        }
    }
    let evaluative = heads.iter().filter(|head| is_evaluative(head)).count();
    heads.len() >= 3 && evaluative >= 2
}

fn is_evaluative(word: &str) -> bool {
    ADJECTIVES.contains(&word)
        || EVALUATIVE_SUFFIXES
            .iter()
            .any(|suffix| word.len() > suffix.len() + 2 && word.ends_with(suffix))
}

/// Title Case: excluding the first word, at least two plain words of four or more letters are
/// capitalised and they make up at least 75 % of such words. Acronyms (`CLI`) and mixed-case
/// names (`GitHub`, `macOS`) are not counted either way. The 75 % bar lets a sentence-case
/// heading carry proper nouns (`Reach Ollama or LM Studio on your Mac`).
pub(crate) fn is_title_case(heading: &str) -> bool {
    let words: Vec<&str> = heading
        .split_whitespace()
        .skip(1)
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| word.chars().count() >= 4 && word.chars().all(char::is_alphabetic))
        .filter(|word| !word.chars().skip(1).any(char::is_uppercase))
        .collect();
    let capitalised = words
        .iter()
        .filter(|word| word.chars().next().is_some_and(char::is_uppercase))
        .count();
    capitalised >= 2 && capitalised * 4 >= words.len() * 3
}

#[cfg(test)]
mod tests {
    use super::{RULES, by_id, is_rule_of_three, is_title_case};

    #[test]
    fn rule_ids_are_unique_kebab_case() {
        for (index, rule) in RULES.iter().enumerate() {
            assert!(
                rule.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{} is not kebab-case",
                rule.id
            );
            assert!(
                RULES
                    .iter()
                    .skip(index + 1)
                    .all(|other| other.id != rule.id),
                "{} is duplicated",
                rule.id
            );
        }
    }

    #[test]
    fn every_rule_has_a_semver_since_and_a_wikipedia_source() {
        for rule in RULES {
            assert_eq!(rule.since.split('.').count(), 3, "{}", rule.id);
            assert!(
                rule.source
                    .starts_with("https://en.wikipedia.org/wiki/Wikipedia:"),
                "{}",
                rule.id
            );
        }
    }

    #[test]
    fn by_id_finds_known_rules_only() {
        assert!(by_id("em-dash").is_some());
        assert!(by_id("no-such-rule").is_none());
    }

    #[test]
    fn rule_of_three_needs_evaluative_items() {
        assert!(is_rule_of_three("fast, reliable, and secure"));
        assert!(is_rule_of_three("is fast, reliable and secure"));
        assert!(!is_rule_of_three("git, curl, and make"));
        assert!(!is_rule_of_three("a path, a mode, and a callback"));
        assert!(!is_rule_of_three("fast and secure"));
    }

    #[test]
    fn title_case_ignores_first_word_acronyms_and_short_words() {
        assert!(is_title_case("Getting Started With Docker Compose"));
        assert!(is_title_case("Why This Approach Matters"));
        assert!(!is_title_case("Getting started with the CLI"));
        assert!(!is_title_case("Install"));
        assert!(!is_title_case("Using GitHub on macOS"));
        assert!(is_title_case("Impact Of Technology And Digitalization"));
        assert!(!is_title_case("Reach Ollama or LM Studio on your Mac"));
        assert!(!is_title_case("Decisions so far"));
    }
}
