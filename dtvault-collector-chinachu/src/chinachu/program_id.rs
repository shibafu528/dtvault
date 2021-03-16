use once_cell::sync::Lazy;
use regex::Regex;
use std::str::FromStr;

const MIRAKURUN_ID_DIVISOR: u64 = 100_000;

const NETWORK_ID_UNKNOWN: u16 = 0;
const NETWORK_ID_BS: u16 = 4;

// {type}{service_id}-(_){radix16_event_id}
static CHINACHU_BETA_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<type>gr|bs|cs|sky)(?P<sid>\d+)-_?(?P<eid>[0-9a-z]+)$").unwrap());

// {radix16_composite_id}(-{radix16_start_at_timestamp})
static CHINACHU_GAMMA_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<id>[0-9a-z]+)(-\d+)?$").unwrap());

#[derive(Debug)]
pub struct ProgramID {
    pub nid: u16,
    pub sid: u16,
    pub eid: u16,
}

#[derive(thiserror::Error, Debug)]
pub enum ParseProgramIDError {
    #[error("id `{0}` does not match the pattern")]
    InvalidFormat(String),
    #[error(transparent)]
    Parse(#[from] std::num::ParseIntError),
}

impl FromStr for ProgramID {
    type Err = ParseProgramIDError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(cap) = CHINACHU_BETA_ID_PATTERN.captures(s) {
            let ch_type = cap.name("type").unwrap().as_str();
            let sid = cap.name("sid").unwrap().as_str().parse()?;
            let nid = chinachu_beta_channel_id_to_nid(ch_type, sid);
            let eid = u16::from_str_radix(cap.name("eid").unwrap().as_str(), 36)?;
            Ok(ProgramID { nid, sid, eid })
        } else if let Some(cap) = CHINACHU_GAMMA_ID_PATTERN.captures(s) {
            let composite_id = u64::from_str_radix(cap.name("id").unwrap().as_str(), 36)?;
            Ok(ProgramID {
                nid: (composite_id / (MIRAKURUN_ID_DIVISOR * MIRAKURUN_ID_DIVISOR)) as u16,
                sid: ((composite_id / MIRAKURUN_ID_DIVISOR) % MIRAKURUN_ID_DIVISOR) as u16,
                eid: (composite_id % MIRAKURUN_ID_DIVISOR) as u16,
            })
        } else {
            Err(ParseProgramIDError::InvalidFormat(s.to_string()))
        }
    }
}

fn chinachu_beta_channel_id_to_nid(ch_type: &str, _sid: u16) -> u16 {
    if ch_type == "bs" {
        NETWORK_ID_BS
    } else {
        NETWORK_ID_UNKNOWN // FIXME: 対応表があればマップできるかも (CSはワンチャン何とかなる、GRは...)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chinachu_beta_gr_program_id() {
        let id1: ProgramID = "gr1064-5xo".parse().unwrap();
        assert_eq!(NETWORK_ID_UNKNOWN, id1.nid);
        assert_eq!(1064, id1.sid);
        assert_eq!(7692, id1.eid);

        // conflicted event id (pre https://github.com/Chinachu/Chinachu/issues/317)
        let id1: ProgramID = "gr1064-_7ih".parse().unwrap();
        assert_eq!(NETWORK_ID_UNKNOWN, id1.nid);
        assert_eq!(1064, id1.sid);
        assert_eq!(9737, id1.eid);
    }

    #[test]
    fn test_parse_chinachu_beta_bs_program_id() {
        let id1: ProgramID = "bs211-14ka".parse().unwrap();
        assert_eq!(NETWORK_ID_BS, id1.nid);
        assert_eq!(211, id1.sid);
        assert_eq!(52570, id1.eid);

        // conflicted event id (pre https://github.com/Chinachu/Chinachu/issues/317)
        let id1: ProgramID = "bs211-_14ka".parse().unwrap();
        assert_eq!(NETWORK_ID_BS, id1.nid);
        assert_eq!(211, id1.sid);
        assert_eq!(52570, id1.eid);
    }

    #[test]
    fn test_parse_chinachu_gamma_gr_program_id() {
        let id1: ProgramID = "3826py3te8".parse().unwrap();
        assert_eq!(32742, id1.nid);
        assert_eq!(1072, id1.sid);
        assert_eq!(26512, id1.eid);
    }

    #[test]
    fn test_parse_chinachu_gamma_bs_program_id() {
        let id1: ProgramID = "idvjnsb".parse().unwrap();
        assert_eq!(NETWORK_ID_BS, id1.nid);
        assert_eq!(211, id1.sid);
        assert_eq!(27723, id1.eid);
    }

    #[test]
    fn test_parse_invalid_id() {
        assert!("hoge65535-xyz".parse::<ProgramID>().is_err());
        assert!("AKAZA_AKARI".parse::<ProgramID>().is_err());
    }
}
