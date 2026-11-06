# Release notes

The build reads its settings from the project file and caches compiled objects between runs, so a second build is much faster than the first one. We moved the cache to a shared directory this week. Teams that run several checkouts on one machine now share compiled dependencies, which saves disk space and time.

The test runner now prints a summary at the end. Failures are grouped by crate, and each group lists the first line of every panic so you can see the cause without scrolling.

Things to check after upgrading:

- Run the full test suite once with a clean cache.
- **Note:** the cache now lives in `target/shared`, so delete the old one by hand.
- Update any scripts that read the old summary format.
- Tell the release channel if a build takes longer than before.

Thanks to everyone who reported slow builds on the old layout. Most of the fixes came from those reports, and the timing data people attached made them much easier to find.
