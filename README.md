# hashall

A simple CLI tool to hash all files in a directory.


# Examples

Hash all files and files inside archives recursively:
```console
hashall . -r --archive
```

Note: `--archive` option only handles `.zip`, `.tar`, `.tar.gz`, and `.tar.zst`

Print in csv format:
```console
hashall . --format csv
```

Single thread only:
```console
hashall . -j 1
```