# hashall
A simple CLI tool to hash all files in a directory.

Hash all files and files inside archives recursively:
```console
hashall . -r --archive
```

Note: `--archive` option only handles `.zip`, `.tar`, and `.tar.gz`

Print in csv format:
```console
hashall . --format csv
```