# Inter-Relay Network (IRN)

## Running

```
$ docker-compose up --build --remove-orphans --scale node_n=20
```

## Troubleshooting

If you see errors like this:

```
#8 0.500 Err:1 http://deb.debian.org/debian bullseye InRelease
#8 0.500   At least one invalid signature was encountered.
```

Clear your docker cache by running:

```
$ docker system prune
```