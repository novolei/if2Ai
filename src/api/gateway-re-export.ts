// MIG-012 — re-export of the MIG-010 gateway bootstrap seam so
// the `src/api/*` barrel is the single import point for every
// bootstrap-time / domain call.
//
// Keeping the actual implementation in `src/transport/gateway.ts`
// preserves the layering set up by MIG-010 (gateway is a
// transport-level primitive, not a domain); this file just
// re-exports.

export {
  awaitGatewayReady,
  getGatewayHealth,
  getGatewayUrl,
  GATEWAY_SCHEMA_VERSION,
  type GatewayHealthPayload,
  type GatewayStatus,
  type GatewayTransport,
  type GatewayUrlPayload,
} from '@/transport/gateway'
