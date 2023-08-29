Feature: Token (interactive) flow

  Background:
    * use karate with config '$regression/config'
    * with karate plugin userinfo

  Scenario: Login without invalid input should redirect to the default error page
    Given url (identityUrl)
    * path '/auth/token/login'
    * params ({rememberMe: "invalid value"})
    When method GET
    Then status 200
    * match page response redirect is 'http://web.scytta-test.com:8080/error?type=invalidInput&status=400'
    * match page response contains 'Failed to deserialize query string'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Login without a token should redirect user to the login page
    Given url (identityUrl)
    * path '/auth/token/login'
    * params (defaultRedirects)
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.loginUrl)
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Login without a token with explicit no-rememberMe should redirect user to the login page
    Given url (identityUrl)
    * path '/auth/token/login'
    * params ({ rememberMe: false, ...defaultRedirects })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.loginUrl)
    * match response 'tid' cookie is removed
    * match response 'sid' cookie is removed
    * match response 'eid' cookie is removed

  Scenario: Login with 'rememberMe' should register a new user
    Given url (identityUrl)
    * path '/auth/token/login'
    * params ({ rememberMe: true, ...defaultRedirects })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie is valid
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) is a guest account

  Scenario: Registering a new user should be able to log in
    Given url (identityUrl)
    * path '/auth/token/login'
    * params ({rememberMe: true, ...defaultRedirects})
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie is valid
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * def userA_TID = (responseCookies.tid.value)
    * def userA_SID = (responseCookies.sid.value)
    * def userA = (await getUserInfo(userA_SID))
    * match user (userA) is a guest account

    Given log ('Trying to register again with a session is an error')
    * url (identityUrl)
    * path '/auth/token/login'
    * params ({ rememberMe: true, ...defaultRedirects })
    * cookies ({sid:userA_SID})
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.errorUrl + '?type=logoutRequired&status=400')
    * match page response contains '&quot;LogoutRequired&quot;'
    * match response 'tid' cookie is removed
    * match response 'sid' cookie has value (userA_SID)
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) equals to (userA)
    
    Given log ('Trying to register again with a session and a token is an error')
    * url (identityUrl)
    * path '/auth/token/login'
    * params (defaultRedirects)
    * cookies ({ sid: userA_SID, tid: userA_TID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.errorUrl + '?type=logoutRequired&status=400')
    * match page response contains '&quot;LogoutRequired&quot;'
    * match response 'tid' cookie has value (userA_TID)
    * match response 'sid' cookie has value (userA_SID)
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) equals to (userA)

    Given log ('Login with the token shall be a success')
    # For test with mismatching token and session please check the auth session tests
    # that performs a much comprehensive validation
    * url (identityUrl)
    * path '/auth/token/login'
    * params (defaultRedirects)
    * cookies ({ tid: userA_TID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie has value (userA_TID)
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) equals to (userA)
    * assert (responseCookies.sid.value !== userA_SID)

    Given log ('Login with the token shall be a success when rememberMe is set')
    * url (identityUrl)
    * path '/auth/token/login'
    * params ({ rememberMe: true, ...defaultRedirects })
    * cookies ({ tid: userA_TID })
    When method GET
    Then status 200
    * match page response redirect is (defaultRedirects.redirectUrl)
    * match response 'tid' cookie has value (userA_TID)
    * match response 'sid' cookie is valid
    * match response 'eid' cookie is removed
    * match user (await getUserInfo(responseCookies.sid.value)) equals to (userA)
    * assert (responseCookies.sid.value !== userA_SID)
