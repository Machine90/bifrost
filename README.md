## Install

### Pingora
We choose pingora as our gateway underlying framework dependencies, before using it, we should install all it related environment:

#### Cmake:

*linux*: 
```shell
sudo apt install cmake=$VERSION (e.g. cmake=3.28.3-1build7)
```

*windows*: We strong recommend you install Cmake by using [chocolately](https://chocolatey.org/install)
```shell
choco install cmake --pre
```

## Features

### Service Discovery
We support 2 kinds service discovery configure, they are:
- Nacos registry services discovery, if use this configure, you must have these environments (or args):
    - `nacos_server_address` for args, `NACOS_SERVER_ADDRESS` for environments, address of your nacos server, e.g. 'https://registry.awesomedomain.com'.
    - `nacos_username` for args, `NACOS_USERNAME` for environment, nacos registry user name, e.g. 'admin'.
    - `nacos_password` for args, `NACOS_PASSWORD` for environment, nacos registry password. e,g, 'mypassword123'

- Static discovery, if use this configure, you should give configure file to let Bifrost know which service and its API should be handled. We support 'toml', 'json' and 'yaml' format configure file, and struct (both single and multiple services are supported) of service looks like:
Json format:
```json
{
    "services": [
        {
            "service_name": "user-svc",
            "api": [
                {
                    "path": "/api/v1/account/register",
                    "roles": [
                        "anonymous"
                    ]
                },
                {
                    "path": "/api/v1/account/login",
                    "roles": [
                        "anonymous"
                    ]
                },
                {
                    "path": "/api/v1/account/current",
                    "roles": [
                        "untagged"
                    ]
                }
            ]
        }
    ]
}
```
or other equivalent toml or yaml:
```toml
[[services]]
service_name = "user-svc"

[[services.api]]
path = "/api/v1/account/register"
roles = [ "anonymous" ]

[[services.api]]
path = "/api/v1/account/login"
roles = [ "anonymous" ]

[[services.api]]
path = "/api/v1/account/current"
roles = [ "untagged" ]
```
and you can specify backend endpoint in the service, for example
```yaml
service: localhost
endpoints:
  - http://localhost:8080
api:
    - path: /static/www/*
      roles:
        - anonymous
```

### Forward requests
This is basic function of Bifrost gateway, which allow to redirect request to internal services, for example we have user service allow user to get their current session, the url is "http://localhost:8081/api/v1/account/current", and its service name is "user-service". then we start a Bifrost gateway and listen on port 8000 at meantime, now we can access this url by using "http://localhost:8000/user-service/api/v1/account/current" (user-service can be also set to header of request).

### Dynamic configure
#### Privilege
todo

#### User roles
todo

#### RateLimit
todo

#### Retry
todo

#### Loadbalance
todo