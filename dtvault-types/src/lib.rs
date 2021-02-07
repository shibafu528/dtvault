pub mod shibafu528 {
    pub mod dtvault {
        tonic::include_proto!("shibafu528.dtvault");

        pub mod central {
            tonic::include_proto!("shibafu528.dtvault.central");
        }

        pub mod storage {
            tonic::include_proto!("shibafu528.dtvault.storage");
        }

        pub mod encoder {
            tonic::include_proto!("shibafu528.dtvault.encoder");
        }
    }
}
