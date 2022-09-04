#!/usr/bin/env python3

import asyncio
import json
from websockets import connect


async def main():
    async with connect("ws://localhost:8081/rpc") as websocket:
        await websocket.send(json.dumps({
            "id": "1",
            "jsonrpc": "2.0",
            "method": "irn_subscribe",
            "params": {
                "topic": "test_topic",
            }
        }))
        print(await websocket.recv())
        await websocket.send(json.dumps({
            "id": "2",
            "jsonrpc": "2.0",
            "method": "irn_publish",
            "params": {
                "topic": "test_topic",
                "message": "hello world!",
                "ttl": 100,
            }
        }))
        print(await websocket.recv())

asyncio.run(main())
