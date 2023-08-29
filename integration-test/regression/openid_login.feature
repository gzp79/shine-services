Feature: OpenId connect flow

  Background:
    * use karate
    * with karate plugin userinfo
    * with karate plugin page

  Scenario: Auth (parameters: NO, cookie: NO) should be an error
    Given Start mock server 'mock' from '$regression/mocks/openid'

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    When method GET
    Then status 200
    * match page response redirect is 'http://web.scytta-test.com:8080/error?type=authError&status=400'
    * match page response contains '&quot;MissingExternalLogin&quot;'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Auth (parameters: VALID, cookie: NO) should be an error
    Given Start mock server 'mock' from '$regression/mocks/openid'

    Given url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * param code = (createUrlQueryString({id: uuidV4(), name: 'n', email: 'n@a.com', nonce: authParams.nonce}))
    * param state = (authParams.state)
    When method GET
    Then status 200
    * match page response redirect is 'http://web.scytta-test.com:8080/error?type=authError&status=400'
    * match page response contains '&quot;MissingExternalLogin&quot;'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Auth (parameters: NO, cookie: VALID) should be an error
    Given Start mock server 'mock' from '$regression/mocks/openid'

    Given url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * cookies ({ eid: userEID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.errorUrl + '?type=invalidInput&status=400')
    * match page response contains 'Failed to deserialize query string'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Auth (parameters: INVALID state, cookie: VALID) should be an error
    Given Start mock server 'mock' from '$regression/mocks/openid'

    Given url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * param code = (createUrlQueryString({id: uuidV4(), name: 'n', email: 'n@a.com', nonce: authParams.nonce}))
    * param state = 'invalid'
    * cookies ({ eid: userEID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.errorUrl + '?type=authError&status=400')
    * match page response contains '&quot;InvalidCSRF&quot;'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Auth (parameters: INVALID code, cookie: VALID) should be an error
    Given Start mock server 'mock' from '$regression/mocks/openid'

    Given url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * param code = "invalid"
    * param state = (authParams.state)
    * cookies ({ eid: userEID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.errorUrl + '?type=authError&status=500')
    * match page response contains 'Server returned empty error response'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Login with failing 3rd party (token service)
    Given url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * param code = (createUrlQueryString({id: uuidV4(), name: 'n', email: 'n@a.com', nonce: authParams.nonce}))
    * param state = (authParams.state)
    * cookies ({ eid: userEID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.errorUrl + '?type=authError&status=500')
    * match page response contains 'No connection could be made because the target machine actively refused it.'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Login with a new user should register a new user
    Given Start mock server 'mock' from '$regression/mocks/openid'

    Given url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * def randomName = (generateRandomString(5))
    * def userSUB = (uuidV4())
    * param code = (createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com', nonce: authParams.nonce}))
    * param state = (authParams.state)
    * cookies ({ eid: userEID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * def userA_SID = (responseCookies.sid.value)
    * def userA = (await getUserInfo(userA_SID))
    * assert (userA.name == randomName)

    Given log ('Login again with the same credentials')
    * url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * param code = (createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com', nonce: authParams.nonce}))
    * param state = (authParams.state)
    * cookies ({ eid: userEID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) equals to (userA)
    * assert (responseCookies.sid.value !== userA_SID)

  Scenario: Login with a new user with rememberMe should register a new user
    Given Start mock server 'mock' from '$regression/mocks/openid'

    Given url (identityUrl)
    * path '/auth/openid_flow/login'
    * params ({rememberMe:true, ...defaultRedirects})
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid
    * def userEID = (responseCookies.eid.value)
    * def authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())

    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * def randomName = (generateRandomString(5))
    * def userSUB = (uuidV4())
    * param code = (createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com', nonce: authParams.nonce}))
    * param state = (authParams.state)
    * cookies ({ eid: userEID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie is valid
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * def userA_TID = (responseCookies.tid.value)
    * def userA_SID = (responseCookies.sid.value)
    * def userA = (await getUserInfo(userA_SID))
    * assert (userA.name == randomName)

    Given log ('Login with session should be an error')
    * url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    * cookies ({ sid: userA_SID, tid: userA_TID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.errorUrl + '?type=logoutRequired&status=400')
    * match page response contains '&quot;LogoutRequired&quot;'
    * match response 'tid' cookie has value (userA_TID)
    * match response 'sid' cookie has value (userA_SID)
    * match response 'eid' cookie is removed

    Given log ('Start of a new login flow with tid should be fine, but both tid shall be removed')
    * url (identityUrl)
    *  path '/auth/openid_flow/login'
    * params ({defaultRedirects})
    * cookies ({ tid: userA_TID })
    When method GET
    Then status 200
    * match page response redirect starts with 'http://mock.localhost.com:8090/openid/authorize'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is valid

    Given log ('Performing a login for the same user shall give the same account')
    * url (identityUrl)
    * path '/auth/openid_flow/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * def userB_EID = (responseCookies.eid.value)
    * def userB_authParams = (getPageRedirectUrl(response).parseQueryParamsFromUrl())
    * assert (userB_authParams.state !== authParams.state)
    Given url (identityUrl)
    * path '/auth/openid_flow/auth'
    * param code = (createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com', nonce: userB_authParams.nonce}))
    * param state = (userB_authParams.state)
    * cookies ({ eid: userB_EID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) equals to (userA)
    * assert (responseCookies.sid.value !== userA_SID)

    Given log ('Performing a token login shall give the same account')
    * url (identityUrl)
    * path '/auth/token/login'
    * params (defaultRedirects)
    * cookies ({ tid: userA_TID})
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie has value (userA_TID)
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) equals to (userA)
    * assert (responseCookies.sid.value !== userA_SID)