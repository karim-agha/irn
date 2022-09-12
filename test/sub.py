#!/usr/bin/env python3

import asyncio
import json
from websockets import connect


async def main():
    async with connect("ws://localhost:8080/rpc") as websocket:
        await websocket.send(json.dumps({
            "id": "3",
            "jsonrpc": "2.0",
            "method": "irn_subscribe",
            "params": {
                "topic": "12D3KooWJA96EVd8JeoLVbeDx7mHsVUc4sWCLQ55a7DpKmv9Cpzj",
            }
        }))
        print(await websocket.recv())
        print(await websocket.recv())
        # TODO Could send an ACK here

asyncio.run(main())
