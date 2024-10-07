import asyncio
import httpx
from dotenv import load_dotenv
import os

load_dotenv()
BITCOIN_RPC = os.getenv('BITCOIN_RPC')
START_HEIGHT = 861295
LOOKBACK = 30
CONCURRENT_REQUESTS = 5

async def get_block_hash(client, height):
    response = await client.post(BITCOIN_RPC, json={
        "jsonrpc": "1.0",
        "id": "curltest",
        "method": "getblockhash",
        "params": [height]
    })
    return response.json()['result']

async def get_block(client, block_hash):
    response = await client.post(BITCOIN_RPC, json={
        "jsonrpc": "1.0",
        "id": "curltest",
        "method": "getblock",
        "params": [block_hash, 1]
    })
    return response.json()['result']

def generate_contract(retarget_block_hash, block_data):
    contract_template = """// SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

contract TestBlocks {{
    bytes32[] public blockHashes;
    uint64[] public blockHeights;
    uint256[] public blockChainworks;
    
    bytes32 public retargetBlockHash;

    constructor() {{
        retargetBlockHash = bytes32(0x{});
        blockHashes = [
{}
        ];
        blockHeights = [
            {}
        ];
        blockChainworks = [
            {}
        ];
    }}
}}"""

    block_hashes = ',\n'.join(f'            bytes32(0x{hash})' for hash, _, _ in block_data)
    block_heights = ', '.join(str(height) for _, height, _ in block_data)
    block_chainworks = ', '.join(str(chainwork) for _, _, chainwork in block_data)

    return contract_template.format(
        retarget_block_hash,
        block_hashes,
        block_heights,
        block_chainworks
    )

async def main():
    async with httpx.AsyncClient() as client:
        end_height = START_HEIGHT + LOOKBACK - 1
        heights = range(START_HEIGHT, end_height + 1)

        # Calculate retarget block height
        retarget_height = START_HEIGHT - (START_HEIGHT % 2016)
        
        semaphore = asyncio.Semaphore(CONCURRENT_REQUESTS)

        async def fetch_block_data(height):
            async with semaphore:
                block_hash = await get_block_hash(client, height)
                block = await get_block(client, block_hash)
                return block_hash, height, int(block['chainwork'], 16)

        # Fetch retarget block hash
        retarget_block_hash = await get_block_hash(client, retarget_height)

        # Fetch main block data
        tasks = [fetch_block_data(height) for height in heights]
        block_data = await asyncio.gather(*tasks)

        # Sort block data by height (should already be in order, but just to be safe)
        block_data.sort(key=lambda x: x[1])

        # Generate Solidity contract
        contract = generate_contract(retarget_block_hash, block_data)

        print("\nGenerated Solidity contract:")
        print(contract)

if __name__ == "__main__":
    asyncio.run(main())
