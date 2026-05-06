# Polymarket API v2 Notes

## Base URLs
- **CLOB API**: `https://clob.polymarket.com`
- **Data API**: `https://data-api.polymarket.com`

## Authentication

### L1 Authentication (Get API Credentials)
1. Sign EIP-712 message with private key
2. POST to `/auth/api-key` with headers:
   - `POLY_ADDRESS`: Polygon signer address
   - `POLY_SIGNATURE`: EIP-712 signature
   - `POLY_TIMESTAMP`: UNIX timestamp
   - `POLY_NONCE`: Nonce (default 0)

Response returns: `apiKey`, `secret`, `passphrase`

### L2 Authentication (API Requests)
For authenticated endpoints, use headers:
- `POLY-API-KEY`: API key
- `POLY-API-SECRET`: Base64 encoded secret
- `POLY-API-PASSPHRASE`: Passphrase

### EIP-712 Signature Structure
```typescript
const domain = {
  name: "ClobAuthDomain",
  version: "1",
  chainId: 137, // Polygon mainnet
};

const types = {
  ClobAuth: [
    { name: "address", type: "address" },
    { name: "timestamp", type: "string" },
    { name: "nonce", type: "uint256" },
    { name: "message", type: "string" },
  ],
};

const value = {
  address: signingAddress,
  timestamp: ts,
  nonce: nonce,
  message: "This message attests that I control the given wallet",
};
```

## Market Data Endpoints (No Auth Required)

### Get Market Prices
- `GET /prices?token_ids=...&sides=...` - Get prices for multiple tokens
- `GET /last-trades-prices?token_ids=...` - Last trade prices
- `GET /midpoints?token_ids=...` - Midpoint prices
- `GET /book?token_id=...` - Order book with bids/asks
- `GET /markets?limit=...&closed=...` - List markets

### Get User Positions/Balance
- `GET /positions?user=0x...` - User positions
- `GET /value?user=0x...` - Total position value

## Trading Endpoints (Require L2 Auth)

### Place Order
- `POST /orders` - Place new order
  - Headers: POLY-API-KEY, POLY-API-SECRET, POLY-API-PASSPHRASE
  - Body: `{ token_id, price, size, side, signature_type }`

### Cancel Order
- `DELETE /orders/{order_id}` - Cancel existing order

### Get Orders
- `GET /orders` - Get user's open orders

## Important Notes

1. Token IDs are 64-character hex strings (without 0x prefix)
2. Prices are in USDC (0.0 to 1.0)
3. Minimum order size varies by market
4. Chain ID is 137 (Polygon mainnet)

## Current Implementation Status

- ✅ BTC price from Binance
- ⚠️ Polymarket prices - API may have changed, needs testing with real token
- ✅ Order placement with balance check
- ✅ INSUFFICIENT_BALANCE error when balance is 0
