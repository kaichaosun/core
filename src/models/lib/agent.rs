use crate::{
    error::{Result, Error},
    models::{
        bank::BankID,
        company::CompanyID,
        company_member::CompanyMemberID,
        region::RegionID,
        user::UserID,
    },
};
use serde::{Serialize, Deserialize};
use std::convert::TryFrom;

/// VF (correctly) assumes different types of actors in the economic network
/// that have "agency" so here we define the objects that have agency within the
/// Basis system. This lets us use a more generic `AgentID` object that fulfills
/// VF's model while still constraining ourselves to a limited set of actors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AgentID {
    #[serde(rename = "bank")]
    BankID(BankID),
    #[serde(rename = "company")]
    CompanyID(CompanyID),
    #[serde(rename = "member")]
    CompanyMemberID(CompanyMemberID),
    #[serde(rename = "region")]
    RegionID(RegionID),
    #[serde(rename = "user")]
    UserID(UserID),
}

/// Implements `From<ModelID> for AgentID` and also `TryFrom<AgentID> for ModelID`
macro_rules! impl_agent_for_model_id {
    ($idty:ident) => {
        impl From<$idty> for AgentID {
            fn from(val: $idty) -> Self {
                AgentID::$idty(val)
            }
        }

        impl TryFrom<AgentID> for $idty {
            type Error = Error;

            fn try_from(val: AgentID) -> Result<Self> {
                Ok(match val {
                    AgentID::$idty(id) => id,
                    _ => Err(Error::WrongAgentIDType)?,
                })
            }
        }
    };
}

impl_agent_for_model_id! { BankID }
impl_agent_for_model_id! { CompanyID }
impl_agent_for_model_id! { CompanyMemberID }
impl_agent_for_model_id! { RegionID }
impl_agent_for_model_id! { UserID }

