import aiohttp
import asyncio

async def check_health():
    async with aiohttp.ClientSession() as session:
        while True:
            try:
                async with session.get('http://localhost:9000/api/health') as response:
                    print(f"Status: {response.status}", flush=True)
                    print(await response.json(), flush=True)
            except Exception as e:
                print(f"Error: {e}", flush=True)
            
            await asyncio.sleep(3)

if __name__ == "__main__":
    asyncio.run(check_health())
