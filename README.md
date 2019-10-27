# Pipe viewer reimplementation
`pv` is a Unix pipe monitoring application. (And this is copy of the much older original)

You can use it in places where a progressbar, or at least a flow rate meter,
would be handy. Some handy examples:

```sh
# Is it still transferring or did something freeze?
docker save excelsior | pv | ssh me@devbox.company.com "docker load"
```

```sh
# Why doesn't gzip have a progressbar already?
pv gigantic-file | gunzip | gawk '/size/ { x += $4 } END {print x}'
```