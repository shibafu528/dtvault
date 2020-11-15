pub mod shibafu528 {
    pub mod dtvault {
        tonic::include_proto!("shibafu528.dtvault");

        pub mod central {
            tonic::include_proto!("shibafu528.dtvault.central");
        }
    }
}
