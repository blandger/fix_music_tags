## CLI utility for fixing music tags in files inside folder 

This is a small utility program to fix 'text tags values' (ID3v2 only) in my old music files I have in my collection. Those files have an incorrect, non utf8 encoding inside tags. The tags **ID3v2** only are supported: title, artist, album, genre, comment.

**Currently only one approach for checking and updating encoding is implemented, but that can be improved by adding more logic.** 

The program scans files in folder, try to check if tags have an incorrect encoding values, reports the results in a 'dry-run' mode by default without physical file changes.
Then you can run it in the mode of modification tags values and updating files on disk. Modification is done by reading, checking invalid encoding, updating value to utf8, writing new tag value to file and writing file back to disk.


### Dev runs

#### Run with info-logs (default)
That only checks if there are incorrect tags values inside music files.

`cargo run -- --dir ./music`

`./fix_music_tags --dir ./music`

### Detailed debug-logs

`RUST_LOG=debug cargo run -- --dir ./music`

`RUST_LOG=info cargo run -- --dir "./music/folder with/empty spaces/"`

`RUST_LOG=info ./fix_music_tags --dir "./music/folder with/empty spaces/"`

#### Real run (then all is OK, otherwise dry-run in code by default)
That command (with --dry-run false ) runs and rewrites.

`cargo run -- --dir "./music/folder with/empty spaces/" --dry-run false`

`./fix_music_tags --dir "./music/folder with/empty spaces/" --dry-run false`