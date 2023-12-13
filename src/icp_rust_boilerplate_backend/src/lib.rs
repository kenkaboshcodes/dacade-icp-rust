#[macro_use]
extern crate serde;

use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};
use std::borrow::Borrow;

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct House {
    id: u64,
    owners_name: String,
    house_type: String,
    location: String,
    created_at: u64,
    price: u64,
    availabile_units: u64,
    availability: bool,
    updated_at: Option<u64>,
    
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct HousePayload {
    owners_name: String,
    house_type: String,
    location: String,
    availabile_units: u64,
    price: u64,
    availability: bool,
}

impl Storable for House {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for House {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE_HOUSE: RefCell<StableBTreeMap<u64, House, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
        ));
}

#[ic_cdk::query]
fn get_house(id: u64) -> Result<House, Error> {
    match _get_house(&id) {
        Some(house) => Ok(house),
        None => Err(Error::NotFound {
            msg: format!("a house with id={} not found", id),
        }),
    }
}

#[ic_cdk::query]
fn get_all_houses() -> Vec<House> {
    STORAGE_HOUSE.with(|service| {
        service
            .borrow()
            .iter()
            .map(|(_, house)| house.clone())
            .collect()
    })
}

#[ic_cdk::query]
fn get_available_houses() -> Vec<House> {
    STORAGE_HOUSE.with(|service| {
        service
            .borrow()
            .iter()
            .filter(|(_, house)| house.availability)
            .map(|(_, house)| house.clone())
            .collect()
    })
}

#[ic_cdk::query]
fn search_houses(query: String) -> Vec<House> {
    STORAGE_HOUSE.with(|service| {
        service
            .borrow()
            .iter()
            .filter(|(_, house)| house.owners_name.contains(&query) || house.house_type.contains(&query))
            .map(|(_, house)| house.clone())
            .collect()
    })
}

#[ic_cdk::query]
fn search_price(query: u64) -> Vec<House> {
    STORAGE_HOUSE.with(|service| {
        service
            .borrow()
            .iter()
            .filter(|(_, house)| house.price == query)
            .map(|(_, house)| house.clone())
            .collect()
    })
}

#[ic_cdk::update]
fn add_house(house: HousePayload) -> Option<House> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    let storage_house = House {
        id,
        owners_name: house.owners_name,
        house_type: house.house_type,
        location: house.location,
        availabile_units: house.availabile_units,
        created_at: time(),
        updated_at: None,
        price: house.price,
        availability: house.availability,
    };
    do_insert_house(&storage_house);
    Some(storage_house)
}

#[ic_cdk::update]
fn update_house(id: u64, payload: HousePayload) -> Result<House, Error> {
    match STORAGE_HOUSE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut house) => {
            house.owners_name = payload.owners_name;
            house.house_type = payload.house_type;
            house.location = payload.location;
            house.updated_at = Some(time());
            house.availabile_units = payload.availabile_units;
            house.price = payload.price;
            house.availability = payload.availability;
            do_insert_house(&house);
            Ok(house.clone())
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update a house with id={}. house not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn buy_house(id: u64, payload: HousePayload) -> Result<House, Error> {
    match STORAGE_HOUSE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut house) => {
            house.owners_name = payload.owners_name;
            house.house_type = payload.house_type;
            house.location = payload.location;
            house.updated_at = Some(time());
            house.availabile_units = payload.availabile_units - 1;
            house.price = payload.price;
            house.availability = payload.availability; 
            do_insert_house(&house);
            Ok(house.clone())
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't buy a house with id={}. house not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn delete_house(id: u64) -> Result<House, Error> {
    match STORAGE_HOUSE.with(|service| service.borrow_mut().remove(&id)) {
        Some(house) => Ok(house),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a house with id={}. house not found.",
                id
            ),
        }),
    }
}

#[ic_cdk::query]
fn house_availability(id: u64) -> Result<bool, Error> {
    match _get_house(&id) {
        Some(house) => Ok(house.availability),
        None => Err(Error::NotFound {
            msg: format!("a house with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn set_house_available(id: u64) -> Result<House, Error> {
    match STORAGE_HOUSE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut house) => {
            house.availability = true;
            do_insert_house(&house);
            Ok(house.clone())
        }
        None => Err(Error::NotFound {
            msg: format!("a house with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn set_house_not_available(id: u64) -> Result<House, Error> {
    if let Some(mut house) = STORAGE_HOUSE.with(|service| service.borrow_mut().get(&id)) {
        house.availability = false;
        do_insert_house(&house);
        Ok(house.clone())
    } else {
        Err(Error::NotFound {
            msg: format!("a house with id={} not found", id),
        })
    }
}

#[ic_cdk::update]
fn set_price(id: u64, price: u64) -> Result<House, Error> {
    match STORAGE_HOUSE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut house) => {
            house.price = price;
            do_insert_house(&house);
            Ok(house.clone())
        }
        None => Err(Error::NotFound {
            msg: format!("a house with id={} not found", id),
        }),
    }
}

fn do_insert_house(house: &House) {
    STORAGE_HOUSE.with(|service| service.borrow_mut().insert(house.id, house.clone()));
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

fn _get_house(id: &u64) -> Option<House> {
    let house_storage = MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)));
    StableBTreeMap::<u64, House, Memory>::init(house_storage)
        .borrow()
        .get(id)
}

#[derive(candid::CandidType, Serialize, Deserialize)]
struct ChangeRecord {
    timestamp: u64,
    change_type: String,
}

#[ic_cdk::query]
fn sort_house_by_name() -> Vec<House> {
    let mut houses = STORAGE_HOUSE.with(|service| {
        service
            .borrow()
            .iter()
            .map(|(_, house)| house.clone())
            .collect::<Vec<_>>()
    });

    houses.sort_by(|a, b| a.owners_name.cmp(&b.owners_name));
    houses
}

#[ic_cdk::query]
fn get_house_update_history(id: u64) -> Vec<ChangeRecord> {
    match _get_house(&id) {
        Some(house) => {
            let mut history = Vec::new();
            if let Some(updated_at) = house.updated_at {
                history.push(ChangeRecord {
                    timestamp: updated_at,
                    change_type: "Update".to_string(),
                });
            }
            history.push(ChangeRecord {
                timestamp: house.created_at,
                change_type: "Creation".to_string(),
            });
            history
        }
        None => Vec::new(),
    }
}

ic_cdk::export_candid!();
