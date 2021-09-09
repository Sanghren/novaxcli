## NovaXCli

WARNING - Pretty much work in progress, use at your own risk : x

This small project aims to automate and ease the management of your NovaX planets.

I can't say enough how annoying it is to manage 10+ planets by hands ...

### How to

I'll try to build a bin so you don't have to compile it yourself, but in the meantime you can run (with rust installed)
the project like this :

- run --package NovaXCli --bin NovaXCli YOUR_ADDRESS PKEY_OF_THIS_ADDRESS GAS_PRICE COMMAND OPTIONAL_COMMAND

Where you have 3 choices for COMMAND :
- fetchInfo
- harvestAll
- upgradeMode
  - `run --package NovaXCli --bin NovaXCli 0x111111 blablabla 85000000000 upgradeMode 2 true true false` <- This will launch the upgrade mode, and it will upgrade buildings on your planets to max level 2 ! If you want to be able to upgrade them to level 3, then change the param to 3 ...
  - the last 3 parameters will enable (or not) the upgrade of respectively : solar / mine and crystal buildings .