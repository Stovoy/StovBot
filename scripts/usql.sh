#!/bin/bash -eu
set -o pipefail
docker run -it -v $(pwd)/db.db3:/db.db3 -v /root/.usql_history:/root/.usql_history stovoy/usql /db.db3
