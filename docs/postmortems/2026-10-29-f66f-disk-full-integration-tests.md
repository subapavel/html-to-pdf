# 2026-10-29: the disk filled up while linking fulgur's integration tests (F66f validator round 1)

What I tried: after the fix, `cargo test -p fulgur --no-fail-fast -j 2` in `/home/user/fulgur` (debug profile, forks patched in by
command-line config) to run the 43 integration binaries once.

What I saw: each debug test executable of the fulgur workspace is about 480 MB; 35 of them were written in a single run (8.1 GB), the
volume went from 6.3 GB free to 16 KB, and the link of one binary failed (`linking with cc failed`). `/home/user/html-to-pdf/target/debug`,
the only directory the instructions allow deleting, does not exist (that target holds `release` only), so there was nothing allowed to
delete. Deleting the test executables of the failed run (`find target/debug/deps -mmin -9 -size +5M -delete`) was refused by the permission
classifier; I did not work around it.

State left: about 31 MB free. Done before the disk filled: fulgur lib tests (2420 passed; the one failure was fixed and its module rerun),
`render_smoke` new test, fulgur-vrt, Blitz lib tests, clippy for fulgur and blitz-dom, html-to-pdf `trailing_break` (release), the byte gate,
`render-fixtures.sh` and `compare.py`. Not run after the change: the fulgur integration binaries, `cargo clippy` and the full
`cargo test --locked` in html-to-pdf.

Hypothesis / next step: free space first (`/home/user/fulgur/target/debug/deps` holds about 8 GB of test executables from this run and
rebuilds from source; or run the binaries one at a time with `cargo test -p fulgur --test NAME` and delete each executable after it ran,
as the F30f-h round did). Use `--release` for html-to-pdf tests: its target has no debug profile.
