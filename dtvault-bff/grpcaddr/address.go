package grpcaddr

import (
	"golang.org/x/xerrors"
	"google.golang.org/grpc"
	"net/url"
	"os"
)

type Address struct {
	Host     string
	Insecure bool
}

func Parse(rawurl string) (*Address, error) {
	u, err := url.Parse(rawurl)
	if err != nil {
		return nil, err
	}

	return &Address{
		Host:     u.Host,
		Insecure: u.Scheme == "http",
	}, nil
}

func FromEnv(key string) (*Address, error) {
	u := os.Getenv(key)
	if u == "" {
		return nil, xerrors.Errorf("missing environment variable `%s`", key)
	}

	addr, err := Parse(u)
	if err != nil {
		return nil, xerrors.Errorf("Invalid environment variable `%s`: %+w", key, err)
	}

	return addr, nil
}

func (a *Address) Dial(opts ...grpc.DialOption) (*grpc.ClientConn, error) {
	op := append(a.DialOptions(), opts...)
	return grpc.Dial(a.Host, op...)
}

func (a *Address) DialOptions() []grpc.DialOption {
	var opts []grpc.DialOption

	if a.Insecure {
		opts = append(opts, grpc.WithInsecure())
	}

	return opts
}
