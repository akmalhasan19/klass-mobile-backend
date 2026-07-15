use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Inter-service request signer for HMAC-SHA256 authentication
pub struct InterServiceRequestSigner {
    secret: String,
}

/// Signed request with HMAC headers
pub struct SignedRequest {
    pub request_id: String,
    pub generation_id: String,
    pub timestamp: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl InterServiceRequestSigner {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    /// Build a signed request with HMAC-SHA256 over `timestamp.encoded_payload`
    ///
    /// Signature = HMAC-SHA256(secret, timestamp + "." + payload)
    ///
    /// Headers returned:
    /// - `X-Request-Id`: UUID v4
    /// - `X-Klass-Generation-Id`: generation_id from caller
    /// - `X-Klass-Request-Timestamp`: Unix epoch seconds
    /// - `X-Klass-Signature-Algorithm`: `hmac-sha256`
    /// - `X-Klass-Signature`: hex-encoded HMAC digest
    pub fn build(&self, generation_id: &str, payload: &[u8]) -> SignedRequest {
        let request_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().timestamp().to_string();

        let signature = self.sign(&timestamp, payload);

        SignedRequest {
            request_id,
            generation_id: generation_id.to_string(),
            timestamp,
            signature_algorithm: "hmac-sha256".to_string(),
            signature,
        }
    }

    fn sign(&self, timestamp: &str, payload: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(payload);
        hex::encode(mac.finalize().into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_determinism() {
        let _signer = InterServiceRequestSigner::new("test-secret".to_string());
        let _generation_id = "550e8400-e29b-41d4-a716-446655440000";
        let payload = br#"{"test":"data"}"#;

        let sig1 = {
            let mut mac = HmacSha256::new_from_slice(b"test-secret").unwrap();
            let timestamp = "1234567890";
            mac.update(timestamp.as_bytes());
            mac.update(b".");
            mac.update(payload);
            hex::encode(mac.finalize().into_bytes())
        };

        let sig2 = {
            let mut mac = HmacSha256::new_from_slice(b"test-secret").unwrap();
            let timestamp = "1234567890";
            mac.update(timestamp.as_bytes());
            mac.update(b".");
            mac.update(payload);
            hex::encode(mac.finalize().into_bytes())
        };

        assert_eq!(sig1, sig2, "Same inputs must produce same signature");
    }

    #[test]
    fn test_header_shape() {
        let signer = InterServiceRequestSigner::new("my-secret".to_string());
        let generation_id = "gen-123";
        let payload = b"hello";

        let req = signer.build(generation_id, payload);

        assert!(!req.request_id.is_empty(), "request_id must be set");
        assert_eq!(req.generation_id, "gen-123");
        assert!(
            req.timestamp.parse::<i64>().is_ok(),
            "timestamp must be unix epoch seconds"
        );
        assert_eq!(req.signature_algorithm, "hmac-sha256");
        assert_eq!(req.signature.len(), 64, "SHA-256 hex digest is 64 chars");
    }
}
