//! Services are processes that require labor but don't create resources.
//!
//! For instance, delivering a package, providing healthcare, or providing legal
//! advice are all services.

use chrono::{DateTime, Utc};
use crate::{
    access::Permission,
    costs::Costs,
    error::{Error, Result},
    models::{
        Op,
        Modifications,
        event::{Event, EventID, EventProcessState},
        company::{Company, Permission as CompanyPermission},
        company_member::CompanyMember,
        process::Process,
        user::User,
    },
};
use vf_rs::vf;

/// Provide a service to another agent, moving costs along the way.
pub fn deliver_service(caller: &User, member: &CompanyMember, company_from: &Company, company_to: &Company, id: EventID, process_from: Process, process_to: Process, move_costs: Costs, now: &DateTime<Utc>) -> Result<Modifications> {
    caller.access_check(Permission::EventCreate)?;
    member.access_check(caller.id(), company_from.id(), CompanyPermission::DeliverService)?;
    if company_from.is_deleted() {
        Err(Error::CompanyIsDeleted)?;
    }
    if company_to.is_deleted() {
        Err(Error::CompanyIsDeleted)?;
    }

    let process_from_id = process_from.id().clone();
    let process_to_id = process_to.id().clone();

    let state = EventProcessState::builder()
        .output_of(process_from)
        .input_of(process_to)
        .build()
        .map_err(|e| Error::BuilderFailed(e))?;
    let event = Event::builder()
        .id(id)
        .inner(
            vf::EconomicEvent::builder()
                .action(vf::Action::DeliverService)
                .has_point_in_time(now.clone())
                .input_of(Some(process_to_id))
                .provider(company_from.id().clone())
                .receiver(company_to.id().clone())
                .output_of(Some(process_from_id))
                .build()
                .map_err(|e| Error::BuilderFailed(e))?
        )
        .move_costs(Some(move_costs))
        .active(true)
        .created(now.clone())
        .updated(now.clone())
        .build()
        .map_err(|e| Error::BuilderFailed(e))?;

    let evmods = event.process(state, now)?.into_vec();
    let mut mods = Modifications::new();
    mods.push(Op::Create, event);
    for evmod in evmods {
        mods.push_raw(evmod);
    }
    Ok(mods)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{
            company::{CompanyID, CompanyType},
            company_member::CompanyMemberID,
            event::{EventID, EventError},
            occupation::OccupationID,
            process::{Process, ProcessID},
            testutils::{make_user, make_company, make_member, make_process},
            user::UserID,
        },
        util,
    };
    use rust_decimal_macros::*;

    #[test]
    fn can_deliver_service() {
        let now = util::time::now();
        let id = EventID::create();
        let company_from = make_company(&CompanyID::create(), CompanyType::Private, "jerry's planks", &now);
        let company_to = make_company(&CompanyID::create(), CompanyType::Private, "jinkey's skateboards", &now);
        let user = make_user(&UserID::create(), None, &now);
        let occupation_id = OccupationID::new("lawyer");
        let member = make_member(&CompanyMemberID::create(), user.id(), company_from.id(), &occupation_id, vec![], &now);
        let process_from = make_process(&ProcessID::create(), company_from.id(), "various lawyerings", &Costs::new_with_labor(occupation_id.clone(), dec!(177.25)), &now);
        let process_to = make_process(&ProcessID::create(), company_to.id(), "employee legal agreement drafting", &Costs::new_with_labor(occupation_id.clone(), dec!(804)), &now);

        let res = deliver_service(&user, &member, &company_from, &company_to, id.clone(), process_from.clone(), process_to.clone(), Costs::new_with_labor("lawyer", 100), &now);
        assert_eq!(res, Err(Error::InsufficientPrivileges));

        let mut member = member.clone();
        member.set_permissions(vec![CompanyPermission::DeliverService]);
        // test ResourceMover::Update()
        let mods = deliver_service(&user, &member, &company_from, &company_to, id.clone(), process_from.clone(), process_to.clone(), Costs::new_with_labor("lawyer", 100), &now).unwrap().into_vec();
        assert_eq!(mods.len(), 3);
        let event = mods[0].clone().expect_op::<Event>(Op::Create).unwrap();
        let process_from2 = mods[1].clone().expect_op::<Process>(Op::Update).unwrap();
        let process_to2 = mods[2].clone().expect_op::<Process>(Op::Update).unwrap();

        assert_eq!(event.id(), &id);
        assert_eq!(event.inner().agreed_in(), &None);
        assert_eq!(event.inner().has_point_in_time(), &Some(now.clone()));
        assert_eq!(event.inner().input_of(), &Some(process_to.id().clone()));
        assert_eq!(event.inner().output_of(), &Some(process_from.id().clone()));
        assert_eq!(event.inner().provider().clone(), company_from.id().clone().into());
        assert_eq!(event.inner().receiver().clone(), company_to.id().clone().into());
        assert_eq!(event.inner().resource_quantity(), &None);
        assert_eq!(event.move_costs(), &Some(Costs::new_with_labor("lawyer", 100)));
        assert_eq!(event.active(), &true);
        assert_eq!(event.created(), &now);
        assert_eq!(event.updated(), &now);

        let mut costs2 = Costs::new();
        costs2.track_labor("lawyer", dec!(177.25) - dec!(100));
        assert_eq!(process_from2.id(), process_from.id());
        assert_eq!(process_from2.company_id(), company_from.id());
        assert_eq!(process_from2.costs(), &costs2);

        let mut costs2 = Costs::new();
        costs2.track_labor("lawyer", dec!(804) + dec!(100));
        assert_eq!(process_to2.id(), process_to.id());
        assert_eq!(process_to2.company_id(), company_to.id());
        assert_eq!(process_to2.costs(), &costs2);

        let user2 = make_user(&UserID::create(), Some(vec![]), &now);
        let res = deliver_service(&user2, &member, &company_from, &company_to, id.clone(), process_from.clone(), process_to.clone(), Costs::new_with_labor("lawyer", 100), &now);
        assert_eq!(res, Err(Error::InsufficientPrivileges));

        let mut member2 = member.clone();
        member2.set_permissions(vec![]);
        let res = deliver_service(&user, &member2, &company_from, &company_to, id.clone(), process_from.clone(), process_to.clone(), Costs::new_with_labor("lawyer", 100), &now);
        assert_eq!(res, Err(Error::InsufficientPrivileges));

        let mut company_from2 = company_from.clone();
        company_from2.set_deleted(Some(now.clone()));
        let res = deliver_service(&user, &member, &company_from2, &company_to, id.clone(), process_from.clone(), process_to.clone(), Costs::new_with_labor("lawyer", 100), &now);
        assert_eq!(res, Err(Error::CompanyIsDeleted));

        let mut company_to2 = company_from.clone();
        company_to2.set_deleted(Some(now.clone()));
        let res = deliver_service(&user, &member, &company_from, &company_to2, id.clone(), process_from.clone(), process_to.clone(), Costs::new_with_labor("lawyer", 100), &now);
        assert_eq!(res, Err(Error::CompanyIsDeleted));

        // can't move costs from a process you don't own
        let mut process_from3 = process_from.clone();
        process_from3.set_company_id(CompanyID::new("zing").into());
        let res = deliver_service(&user, &member, &company_from, &company_to, id.clone(), process_from3.clone(), process_to.clone(), Costs::new_with_labor("lawyer", 100), &now);
        assert_eq!(res, Err(Error::Event(EventError::ProcessOwnerMismatch)));

        // can't move costs into a process company_to doesnt own
        let mut process_to3 = process_to.clone();
        process_to3.set_company_id(CompanyID::new("zing").into());
        let res = deliver_service(&user, &member, &company_from, &company_to, id.clone(), process_from.clone(), process_to3.clone(), Costs::new_with_labor("lawyer", 100), &now);
        assert_eq!(res, Err(Error::Event(EventError::ProcessOwnerMismatch)));
    }
}
