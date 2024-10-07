import asyncio
import httpx
from dotenv import load_dotenv
import os
import json

load_dotenv()
BITCOIN_RPC = os.getenv('BITCOIN_RPC')
BLOCK_HEIGHTS = [854784, 856799, 856800, 856801, 854376, 852768, 854373, 854374, 854375, 854377, 854378, 854379, 854380, 854136, 858564, 858565, 858566, 858567, 858568]
CONCURRENT_REQUESTS = 10
OUTPUT_DIR = "tests/data"

async def get_block_hash(client, height):
    response = await client.post(BITCOIN_RPC, json={
        "jsonrpc": "1.0",
        "id": "curltest",
        "method": "getblockhash",
        "params": [height]
    })
    return response.json()['result']

async def get_block(client, block_hash) -> str: 
    response = await client.post(BITCOIN_RPC, json={
        "jsonrpc": "1.0",
        "id": "curltest",
        "method": "getblock",
        "params": [block_hash, 0]
    })
    return response.json()['result']

async def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    
    async with httpx.AsyncClient() as client:
        semaphore = asyncio.Semaphore(CONCURRENT_REQUESTS)

        async def fetch_block_data(height):
            async with semaphore:
                output_file = os.path.join(OUTPUT_DIR, f"block_{height}.hex")
                if os.path.exists(output_file):
                    print(f"Block {height} already exists. Skipping.")
                    return None

                block_hash = await get_block_hash(client, height)
                block = await get_block(client, block_hash)
                
                with open(output_file, 'w') as f:
                    f.write(block)
                
                print(f"Block {height} saved successfully.")

        tasks = [fetch_block_data(height) for height in BLOCK_HEIGHTS]
        await asyncio.gather(*tasks)
        print("All blocks saved successfully.")


if __name__ == "__main__":
    asyncio.run(main())
