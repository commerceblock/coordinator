# Coordinator

Implementation of the Coordinator daemon, responsible for verifying the operation of Guardnodes in the Commerceblock Covalence system


## Instructions

### Run Coordinator

To run a production instance of the coordinator along with a mongo db database, edit the envs in the [docker compose file](https://github.com/commerceblock/coordinator/blob/develop/docker-compose.yml) and:

`docker-compose up`

Or to test the coordinator locally:

`cargo run`


### Run Demo

Check out the demo [here](https://commerceblock.readthedocs.io/en/latest/coordinator/index.html#demo).


### Docs

For more details check [readthedocs](https://commerceblock.readthedocs.io/en/latest/coordinator/index.html).
