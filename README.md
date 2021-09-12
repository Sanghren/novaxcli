## NovaXCli

WARNING - Pretty much work in progress, use at your own risk : x

This small project aims to automate and ease the management of your NovaX planets.

[NovaX](https://novaxgame.com/) is a game running on [Avalanche](https://docs.avax.network/)'s C Chain.

I can't say enough how annoying it is to manage 10+ planets by hands ...

### How to run it

For now the only way to run NOvaXCli is by cloning the repo locally and building it from source.
For that you will need to setup Rust (https://www.rust-lang.org/tools/install)

Once this is done, go into the folder, and run `cargo run --package NovaXCli --bin NovaXCli YOUR_ADDRESS PRIVATE_KEY_OF_THIS_ADDRESS GAS_PRICE_IN_WEI COMMAND (OPTIONAL_COMMAND`

Here's a quick overview of the different commands you can run.

#### fetchInfo

`cargo run --package NovaXCli --bin NovaXCli YOUR_ADDRESS PRIVATE_KEY_OF_THIS_ADDRESS GAS_PRICE_IN_WEI fetchInfo`


This command will go over all your planets, fetching the pending resources and display that to you.

#### harvestAll

`cargo run --package NovaXCli --bin NovaXCli YOUR_ADDRESS PRIVATE_KEY_OF_THIS_ADDRESS GAS_PRICE_IN_WEI harvestAll`

This command will simply trigger a call to the `harvestAll` function.

#### upgradeMode

`cargo run --package NovaXCli --bin NovaXCli YOUR_ADDRESS PRIVATE_KEY_OF_THIS_ADDRESS GAS_PRICE_IN_WEI upgradeMode 3 true true true`

This command will trigger an upgrade on the buildings of your planets. It will only upgrade the buildings that are below
a certain level (the last parameter in the example command above) .

You can also precise which building you want to upgrade (the 3 boolean parameters in the command above).

Where the first one is for the solar building, second one for the mine and the last one for the crystal lab.

### ToDo
- [] Build a bin
- [] Experiment with [Rust Tui](https://github.com/fdehau/tui-rs)
- [] Improve the logging a bit more

### Donation

This tool is entirely free, if you wanna say 'thanks' here's my Avalanche's CCHain address -> 0x19E13130738568a964f7C7Eb5D11fdc72271ae0F .
