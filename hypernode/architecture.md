On start, connect to ethereum and bitcoin rpc nodes, download all active reservations from the rift exchange contract and store it in memory.
Take the oldest reservations timestamp, and find what bitcoin block was before that timestamp (maybe -1 hour to be safe).
Then for every bitcoin block between then and the current height,
search for transactions that include an OP_RETURN output with an active reservation's order nonce.
If a btc transaction is found, validate that it's outputs match with the reservation and store the bitcoin txid and associated reservation in memory, this implies that the reservation is waiting for block confirmations


Then simultaneously poll bitcoin for new blocks, and the rift exchange contract on ethereum for reservation updating events. 
On each evm event():
=> If a reservation is created, download it and add it to the list of active reservations.
=> If a reservation is expired, remove it from the list of active reservations.
=> If a reservation is fullfilled, remove it from the list of active reservations.
When a bitcoin block is mined:
=> Search each transaction in the block for a reservation payment, 
=> Check if any of the reservations waiting to be confirmed have had enough blocks mined to be considered confirmed.
=> Remove any reservations that expired from the in memory list
