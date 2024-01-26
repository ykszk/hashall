# hashall
A simple CLI tool to hash all files in a directory.

Hash all files (including files inside archives) recursively:
```console
hashall . -r --archive
```

Note: `--archive` only handles zip archives.

Print in csv format:
```console
hashall . --format csv
```