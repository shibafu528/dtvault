package graph

import "github.com/shibafu528/dtvault/dtvault-bff/grpcaddr"

//go:generate go run github.com/99designs/gqlgen

// This file will not be regenerated automatically.
//
// It serves as dependency injection for your app, add any dependencies you require here.

type Resolver struct {
	CentralAddr *grpcaddr.Address
	EncoderAddr *grpcaddr.Address
}
