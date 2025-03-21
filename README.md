# BitFusion SDK

An open-source framework designed to enable the creation of decentralized bridges between EVM networks and Bitcoin, Bitcoin Runes, and other EVM networks, leveraging threshold cryptography and a network of on-chain Bitcoin nodes.

## Warning

This code is still in the early stages of development and is not intended for production use. Use at your own risk.

## Deploy

See [Deploy readme](./docs/deploy.md)

## Running tests

To run tests you can use `just`.

Integration tests can be either run against the Bitfinity EVM, if you have access to the Bitfinity evm repository or against a local instance of ganache which is automatically started by the test runner.

In case you need to test with the local ganache instance you must set this environment variable:

```bash
export EVM=ganache
```

and then tests can be run with

```bash
just test
just integration_test
just dfx_test
```

## License

[MIT](https://choosealicense.com/licenses/mit/)

## Legal notice

Finity Technologies encourages developers to evaluate their own regulatory obligations when using this code, including, but not limited to, those related to compliance.

THE CODE (THE “CODE”) PROVIDED BY FINITY TECHNOLOGIES LTD (THE “COMPANY”) IS PROVIDED ON AN AS IS BASIS. THE COMPANY DOES NOT GIVE ANY WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF FITNESS FOR A PARTICULAR PURPOSE AND/OR NONINFRINGEMENT. IN NO EVENT SHALL THE COMPANY BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE CODE, AND/OR THE USE OF OR OTHER DEALINGS IN THE CODE.

USERS SHOULD BE AWARE OF THE RISKS ASSOCIATED WITH USING OPEN-SOURCE CODE WHICH INCLUDE, BUT ARE NOT LIMITED TO, LACK OF SECURITY, OPERATIONAL INSUFFICIENCIES, SOFTWARE QUALITY. BY USING THE CODE USERS ACCEPT THESE RISKS. FOR THE AVOIDANCE OF DOUBT, THE COMPANY IS NOT RESPONSIBLE FOR AND ACCEPTS NO LIABILITY FOR ANY LOSS WHICH RESULTS FROM ANY SUCH RISK MATERIALISING OR ANY OTHER ISSUE WITH THE CODE.
