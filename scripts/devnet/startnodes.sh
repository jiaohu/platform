#!/usr/bin/env bash

RED='\033[31m'
GRN="\033[32m"
NC='\033[0m'

# paths
DEVNET="$LEDGER_DIR/devnet"

# start abcis
nodes=`ls -l $DEVNET | grep node  | awk '(NR>0){print $9}' | sort -V`
for node in $nodes
do
    abci_validator_node $DEVNET/$node >> $DEVNET/$node/abci_validator.log 2>&1  &
done

# start nodes
for node in $nodes
do
    tendermint node --home $DEVNET/$node >> $DEVNET/$node/consensus.log 2>&1  &
done

# show abcis and nodes
for node in $nodes
do
    echo -n "$node: "
    abci=`pgrep -f "abci_validator_node $DEVNET/$node$" | tr "\n" " " | xargs echo -n`
    echo -en "abci(${GRN}$abci${NC}) <---> "
    sleep 1
    node=`pgrep -f "tendermint node --home $DEVNET/$node$" | tr "\n" " " | xargs echo -n`
    echo -e "node(${GRN}$node${NC})"
done