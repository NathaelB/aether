#!/bin/bash
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    -- Create ferriskey database
    CREATE DATABASE ferriskey;
    GRANT ALL PRIVILEGES ON DATABASE ferriskey TO $POSTGRES_USER;
EOSQL

echo "✅ Created ferriskey database"
