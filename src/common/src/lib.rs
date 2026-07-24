/*
 * SPDX-FileCopyrightText: Copyright 2024 LG Electronics Inc.
 * SPDX-License-Identifier: Apache-2.0
 */
pub use crate::error::Result;

pub mod error;
pub mod etcd;
pub mod setting;
pub mod spec;

// gRPC protobuf module for RocksDB service
pub mod rocksdbservice {
    include!("generated/rocksdbservice.rs");
}

fn open_server(port: u16) -> String {
    format!("{}:{}", crate::setting::get_config().host.ip, port)
}

// // guest 서버 함수 수정: 이제 항상 호스트 서버 주소 반환
// fn open_guest_server(port: u16) -> String {
//     // 항상 호스트 서버 주소 반환
//     open_server(port)
// }

fn connect_server(port: u16) -> String {
    format!("http://{}:{}", crate::setting::get_config().host.ip, port)
}

// guest 서버 연결 함수 수정: 이제 항상 호스트 서버 주소 반환
//using rust build in _ to below code , as it was never used anywhere to prevent warnings
fn _connect_guest_server(port: u16) -> String {
    // 항상 호스트 서버 주소 반환
    connect_server(port)
}

pub mod actioncontroller {
    include!("generated/actioncontroller.rs");

    pub fn open_server() -> String {
        super::open_server(47001)
    }

    pub fn connect_server() -> String {
        super::connect_server(47001)
    }
}

pub mod apiserver {
    include!("generated/apiserver.rs");

    pub fn open_rest_server() -> String {
        super::open_server(47099)
    }

    pub fn open_grpc_server() -> String {
        super::open_server(47098)
    }

    pub fn connect_grpc_server() -> String {
        super::connect_server(47098)
    }
}

pub mod filtergateway {
    include!("generated/filtergateway.rs");

    pub fn open_server() -> String {
        super::open_server(47002)
    }

    pub fn connect_server() -> String {
        super::connect_server(47002)
    }
}

pub mod monitoringserver {
    include!("generated/monitoringserver.rs");

    pub fn open_server() -> String {
        super::open_server(47003)
    }

    pub fn connect_server() -> String {
        super::connect_server(47003)
    }
}

pub mod nodeagent {
    include!("generated/nodeagent.rs");

    pub mod fromactioncontroller {
        include!("generated/nodeagent.fromactioncontroller.rs");

        pub fn connect_server(node_ip: &str) -> String {
            format!("http://{node_ip}:47004")
        }
    }

    pub mod fromapiserver {
        include!("generated/nodeagent.fromapiserver.rs");
    }
}

pub mod policymanager {
    include!("generated/policymanager.rs");

    pub fn open_server() -> String {
        super::open_server(47005)
    }

    pub fn connect_server() -> String {
        super::connect_server(47005)
    }
}

pub mod statemanager {
    include!("generated/statemanager.rs");

    pub fn open_server() -> String {
        super::open_server(47006)
    }

    pub fn connect_server() -> String {
        super::connect_server(47006)
    }
}

pub mod logd;

pub mod external {
    pub mod timpani {
        include!("generated/schedinfo.v1.rs");
        pub fn connect_timpani_server() -> String {
            format!("http://{}:{}", crate::setting::get_config().host.ip, 50052)
        }
    }

    pub mod pharos {
        include!("generated/pharos.api.v1.rs");
        pub fn connect_pharos_server() -> String {
            format!("http://{}:{}", crate::setting::get_config().host.ip, 47006)
        }
    }
}

//Unit Test Cases
#[cfg(test)]
mod tests {
    #[test]
    fn test_open_server_direct() {
        let res = crate::open_server(8080);
        assert!(res.contains(":8080"));
    }

    #[test]
    fn test_connect_server_direct() {
        let res = crate::connect_server(8080);
        assert!(res.starts_with("http://"));
        assert!(res.contains(":8080"));
    }

    #[test]
    fn test_connect_guest_server() {
        let res = crate::_connect_guest_server(8080);
        assert!(res.contains(":8080"));
    }

    #[test]
    fn test_module_server_endpoints() {
        assert!(crate::actioncontroller::open_server().contains(":47001"));
        assert!(crate::actioncontroller::connect_server().contains(":47001"));

        assert!(crate::apiserver::open_rest_server().contains(":47099"));
        assert!(crate::apiserver::open_grpc_server().contains(":47098"));
        assert!(crate::apiserver::connect_grpc_server().contains(":47098"));

        assert!(crate::filtergateway::open_server().contains(":47002"));
        assert!(crate::filtergateway::connect_server().contains(":47002"));

        assert!(crate::monitoringserver::open_server().contains(":47003"));
        assert!(crate::monitoringserver::connect_server().contains(":47003"));

        assert_eq!(
            crate::nodeagent::fromactioncontroller::connect_server("192.168.1.10"),
            "http://192.168.1.10:47004"
        );

        assert!(crate::policymanager::open_server().contains(":47005"));
        assert!(crate::policymanager::connect_server().contains(":47005"));

        assert!(crate::statemanager::open_server().contains(":47006"));
        assert!(crate::statemanager::connect_server().contains(":47006"));

        assert!(crate::external::timpani::connect_timpani_server().contains(":50052"));
        assert!(crate::external::pharos::connect_pharos_server().contains(":47006"));
    }
}
