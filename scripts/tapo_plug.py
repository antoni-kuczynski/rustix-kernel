# MIT License
#
# Copyright (c) 2022-2025 Mihai Dinculescu
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

# This script is used for powering on and off tapo smart plug.
# I have a p110 so thats the one used here
# Requires a .venv set up inside scripts/.venv and tapo package installed

import asyncio
import sys

from tapo import ApiClient

tapo_email = "tapo_email"
tapo_password = "tapo_password"
device_ip = "smart_plug_id"

client = ApiClient(tapo_email, tapo_password)
# ============================================================================
async def power_on():
    device = await client.p110(device_ip)
    await device.on()
# ============================================================================
async def power_off():
    device = await client.p110(device_ip)
    await device.off()
# ============================================================================
async def main():
    if len(sys.argv) > 1:
        command = sys.argv[1]

        if command == "on":
            await power_on()
        elif command == "off":
            await power_off()
        else:
            print(f"Invalid command {command}! Expected \"on\" | \"off\".")
    else:
        print("Provide an argument first! Expected \"on\" | \"off\".")
# ============================================================================
if __name__ == "__main__":
    asyncio.run(main())
