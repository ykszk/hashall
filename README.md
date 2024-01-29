# hashall

A simple CLI tool to hash all files in a directory (and files in the archive files).


# Examples

Hash all files and files inside archives recursively:
```console
hashall . -r --archive
```

Note: `--archive` option handles `.zip`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz`, and `.tar.zst`

Print in csv format:
```console
hashall . --format csv
```

Single thread only:
```console
hashall . -j 1
```