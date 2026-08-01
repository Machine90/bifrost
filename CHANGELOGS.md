# CHANGELOGS

## [20260731 MR-1](https://github.com/Machine90/bifrost/pull/1)
**Features**
- Support to query user privilege and info by using JWT of Authorization header.

**Bugfix**
- Fixed issue of getting incorrect user roles when querying user privileges.

**Optimize**
- Added json type error response with error message.

### Bump version:
- bifrost: 0.1.8

## [20260728]
**Changes**
- Added API function for getting current users privilege.

### Bump version:
- bifrost: 0.1.7

## [20260701]
**Changes**
- Fixed some issues of user privilege check.
- Added `ListUserConfigsByIds` API.
- Fixed issues of api router cache operations and added unittest for it.

### Bump version:
- bifrost: 0.1.6

## [20260410]
**Changes**
- Fixed language issue of querying roles and platform.
- Make roles API allow logged-in user role to access.

### Bump version:
- bifrost: 0.1.5

## [20260326]
**Changes**
- Add sentry & tracing configure.
- Fixed issue of removing platform from routes

### Bump version:
- bifrost: 0.1.4

## [20260326]
**Changes**
- Optimize roles display.
- Fixed issue of user role and url privilege match.
- Support forward logged-in user roles to downstream services.
- Support verify user identity from configured server.

### Bump version:
- bifrost: 0.1.3

## [20260324]
**Changes**
- Support SSL proxy and cert management.
- Add tracing extension.

### Bump version:
- bifrost: 0.1.2

## [20260324]
**Changes**:
- Add method (GET, POST, PUT, DELETE, HEAD, PUT) mark for each api configures path.

### Bump version:
- bifrost: 0.1.1
