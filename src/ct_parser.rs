use crate::types::*;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::collections::HashMap;
use x509_parser::prelude::*;

pub fn parse_ct_entry(entry: &CTLogEntry) -> Result<(LeafCertificate, Vec<ChainCertificate>)> {
    let leaf_input = BASE64.decode(&entry.leaf_input)?;
    let extra_data = BASE64.decode(&entry.extra_data)?;

    // Parse leaf input structure
    if leaf_input.len() < 11 {
        return Err(anyhow!("Leaf input too short"));
    }

    let entry_type = u16::from_be_bytes([leaf_input[9], leaf_input[10]]);
    let entry_data = &leaf_input[11..];

    let _update_type = match entry_type {
        0 => "X509LogEntry",
        1 => "PrecertLogEntry",
        _ => "Unknown",
    };

    let (leaf_cert, chain) = match entry_type {
        0 => parse_x509_entry(entry_data, &extra_data)?,
        1 => parse_precert_entry(entry_data, &extra_data)?,
        _ => return Err(anyhow!("Unknown entry type: {}", entry_type)),
    };

    Ok((leaf_cert, chain))
}

fn parse_x509_entry(
    entry_data: &[u8],
    extra_data: &[u8],
) -> Result<(LeafCertificate, Vec<ChainCertificate>)> {
    if entry_data.len() < 3 {
        return Err(anyhow!("Entry data too short"));
    }

    let cert_len = u32::from_be_bytes([0, entry_data[0], entry_data[1], entry_data[2]]) as usize;
    if entry_data.len() < 3 + cert_len {
        return Err(anyhow!("Certificate data truncated"));
    }

    let cert_data = &entry_data[3..3 + cert_len];
    let leaf_cert = parse_certificate(cert_data, true)?;

    // Parse chain from extra_data
    let chain = parse_certificate_chain(extra_data)?;

    Ok((leaf_cert, chain))
}

fn parse_precert_entry(
    _entry_data: &[u8],
    extra_data: &[u8],
) -> Result<(LeafCertificate, Vec<ChainCertificate>)> {
    if extra_data.len() < 3 {
        return Err(anyhow!("Extra data too short"));
    }

    let cert_len = u32::from_be_bytes([0, extra_data[0], extra_data[1], extra_data[2]]) as usize;
    if extra_data.len() < 3 + cert_len {
        return Err(anyhow!("Certificate data truncated"));
    }

    let cert_data = &extra_data[3..3 + cert_len];
    let leaf_cert = parse_certificate(cert_data, true)?;

    // Parse additional chain
    let chain_start = 3 + cert_len;
    let chain = if chain_start < extra_data.len() {
        parse_certificate_chain(&extra_data[chain_start..])?
    } else {
        Vec::new()
    };

    Ok((leaf_cert, chain))
}

fn parse_certificate(der_data: &[u8], is_leaf: bool) -> Result<LeafCertificate> {
    let (_, cert) = X509Certificate::from_der(der_data)
        .map_err(|e| anyhow!("Failed to parse certificate: {:?}", e))?;

    let subject = parse_subject(&cert.subject);
    let extensions = parse_extensions(&cert);
    
    let not_before = cert.validity().not_before.timestamp() as f64;
    let not_after = cert.validity().not_after.timestamp() as f64;

    let all_domains = if is_leaf {
        extract_all_domains(&cert)
    } else {
        Vec::new()
    };

    let as_der = Some(BASE64.encode(der_data));

    Ok(LeafCertificate {
        subject,
        extensions,
        not_before,
        not_after,
        as_der,
        all_domains,
    })
}

fn parse_chain_certificate(der_data: &[u8]) -> Result<ChainCertificate> {
    let (_, cert) = X509Certificate::from_der(der_data)
        .map_err(|e| anyhow!("Failed to parse certificate: {:?}", e))?;

    let subject = parse_subject(&cert.subject);
    let extensions = parse_extensions(&cert);
    
    let not_before = cert.validity().not_before.timestamp() as f64;
    let not_after = cert.validity().not_after.timestamp() as f64;
    let as_der = Some(BASE64.encode(der_data));

    Ok(ChainCertificate {
        subject,
        extensions,
        not_before,
        not_after,
        as_der,
    })
}

fn parse_certificate_chain(mut data: &[u8]) -> Result<Vec<ChainCertificate>> {
    let mut chain = Vec::new();

    // Skip initial chain length if present
    if data.len() >= 3 {
        let chain_len = u32::from_be_bytes([0, data[0], data[1], data[2]]) as usize;
        if chain_len > 0 && data.len() >= 3 + chain_len {
            data = &data[3..];
        } else if chain_len == 0 {
            data = &data[3..];
        }
    }

    while data.len() >= 3 {
        let cert_len = u32::from_be_bytes([0, data[0], data[1], data[2]]) as usize;
        if data.len() < 3 + cert_len {
            break;
        }

        let cert_data = &data[3..3 + cert_len];
        if let Ok(chain_cert) = parse_chain_certificate(cert_data) {
            chain.push(chain_cert);
        }

        data = &data[3 + cert_len..];
    }

    Ok(chain)
}

fn parse_subject(subject: &X509Name) -> Subject {
    let mut c = None;
    let mut st = None;
    let mut l = None;
    let mut o = None;
    let mut ou = None;
    let mut cn = None;

    for rdn in subject.iter() {
        for attr in rdn.iter() {
            if let Ok(value) = attr.attr_value().as_str() {
                if attr.attr_type() == &oid_registry::OID_X509_COUNTRY_NAME {
                    c = Some(value.to_string());
                } else if attr.attr_type() == &oid_registry::OID_X509_STATE_OR_PROVINCE_NAME {
                    st = Some(value.to_string());
                } else if attr.attr_type() == &oid_registry::OID_X509_LOCALITY_NAME {
                    l = Some(value.to_string());
                } else if attr.attr_type() == &oid_registry::OID_X509_ORGANIZATION_NAME {
                    o = Some(value.to_string());
                } else if attr.attr_type() == &oid_registry::OID_X509_ORGANIZATIONAL_UNIT {
                    ou = Some(value.to_string());
                } else if attr.attr_type() == &oid_registry::OID_X509_COMMON_NAME {
                    cn = Some(value.to_string());
                }
            }
        }
    }

    let aggregated = format!(
        "{}{}{}{}{}{}",
        c.as_ref().map(|v| format!("/C={}", v)).unwrap_or_default(),
        st.as_ref().map(|v| format!("/ST={}", v)).unwrap_or_default(),
        l.as_ref().map(|v| format!("/L={}", v)).unwrap_or_default(),
        o.as_ref().map(|v| format!("/O={}", v)).unwrap_or_default(),
        ou.as_ref().map(|v| format!("/OU={}", v)).unwrap_or_default(),
        cn.as_ref().map(|v| format!("/CN={}", v)).unwrap_or_default(),
    );

    Subject {
        aggregated,
        c,
        st,
        l,
        o,
        ou,
        cn,
    }
}

fn parse_extensions(cert: &X509Certificate) -> HashMap<String, String> {
    let mut extensions = HashMap::new();

    for ext in cert.extensions() {
        let value = format!("{:?}", ext.parsed_extension());

        // Match common OIDs using string comparison since Oid doesn't implement PartialEq
        let key = if ext.oid == x509_parser::oid_registry::OID_X509_EXT_KEY_USAGE {
            "keyUsage"
        } else if ext.oid == x509_parser::oid_registry::OID_X509_EXT_EXTENDED_KEY_USAGE {
            "extendedKeyUsage"
        } else if ext.oid == x509_parser::oid_registry::OID_X509_EXT_BASIC_CONSTRAINTS {
            "basicConstraints"
        } else if ext.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_KEY_IDENTIFIER {
            "subjectKeyIdentifier"
        } else if ext.oid == x509_parser::oid_registry::OID_X509_EXT_AUTHORITY_KEY_IDENTIFIER {
            "authorityKeyIdentifier"
        } else if ext.oid == x509_parser::oid_registry::OID_PKIX_AUTHORITY_INFO_ACCESS {
            "authorityInfoAccess"
        } else if ext.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME {
            "subjectAltName"
        } else if ext.oid == x509_parser::oid_registry::OID_X509_EXT_CERTIFICATE_POLICIES {
            "certificatePolicies"
        } else if ext.oid == x509_parser::oid_registry::OID_X509_EXT_CRL_DISTRIBUTION_POINTS {
            "crlDistributionPoints"
        } else {
            continue;
        };

        extensions.insert(key.to_string(), value);
    }

    extensions
}

fn extract_all_domains(cert: &X509Certificate) -> Vec<String> {
    let mut domains = Vec::new();

    // Get CN from subject
    for rdn in cert.subject.iter() {
        for attr in rdn.iter() {
            if attr.attr_type() == &oid_registry::OID_X509_COMMON_NAME {
                if let Ok(cn) = attr.attr_value().as_str() {
                    domains.push(cn.to_string());
                }
            }
        }
    }

    // Get SANs from extensions
    for ext in cert.extensions() {
        if ext.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME {
            let san = ext.parsed_extension();
            if let x509_parser::extensions::ParsedExtension::SubjectAlternativeName(san) = san {
                for name in &san.general_names {
                    if let x509_parser::extensions::GeneralName::DNSName(dns) = name {
                        domains.push(dns.to_string());
                    }
                }
            }
        }
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    domains.retain(|d| seen.insert(d.clone()));

    domains
}
