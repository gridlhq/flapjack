package transport

import (
	"time"

	"github.com/flapjackhq/flapjack-search-go/v4/flapjack/compression"
)

type Configuration struct {
	AppID  string
	ApiKey string //nolint:staticcheck

	Hosts                           []StatefulHost
	DefaultHeader                   map[string]string
	UserAgent                       string
	Requester                       Requester
	ReadTimeout                     time.Duration
	WriteTimeout                    time.Duration
	ConnectTimeout                  time.Duration
	Compression                     compression.Compression
	ExposeIntermediateNetworkErrors bool
}

type RequestConfiguration struct {
	ReadTimeout    *time.Duration
	WriteTimeout   *time.Duration
	ConnectTimeout *time.Duration
}
