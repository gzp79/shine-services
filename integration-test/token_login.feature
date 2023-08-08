Feature: Token credentials

  Background:
    * url karate.properties['identityUrl']
    * def redirects =
    """
    {
        loginUrl: 'http://login.com',
        redirectUrl: 'http://redirect.com',
        errorUrl: 'http://error.com'
    }
    """

  Scenario: Login without a token should redirect user to the login page
    Given path '/auth/token/login'
      * params redirects
      * configure cookies = null
      * method get
    Then status 200
      * match responseCookies.tid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.eid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.sid contains {path: "/", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match response contains 'http://login.com'
      * match response !contains 'http://redirect.com'
      * match response !contains 'http://error.com'

  Scenario: Login with 'rememberMe' should register a new user
    Given path '/auth/token/login'
      * params redirects
      * param rememberMe = true
      * configure cookies = null
      * method get
    Then status 200
      * match responseCookies.tid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true}
      * match responseCookies.eid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.sid contains {path: "/", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true}
      * match response contains 'http://redirect.com'
      * match response !contains 'http://login.com'
      * match response !contains 'http://error.com'
    And def userA_SID = responseCookies.sid.value

    # Waiting to check session length too
    Given eval java.lang.Thread.sleep(1000)
      * def utils = call read('utils/userinfo.feature') {userSession: #(userA_SID)}
      * match utils.userInfo contains {name: #? _.startsWith('Freshman_'), sessionLength: #? _ >= 1}

  Scenario: Registering a new user should be able to log in
    Given path '/auth/token/login'
      * params redirects
      * param rememberMe = true
      * configure cookies = null
      * method get
    Then status 200
      * match responseCookies.tid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true}
      * match responseCookies.eid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.sid contains {path: "/", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true}
      * match response contains 'http://redirect.com'
      * match response !contains 'http://login.com'
      * match response !contains 'http://error.com'
    And def userA_SID = responseCookies.sid.value
      * def userA_TID = responseCookies.tid.value
    
    # Getting user info shall be success
    Given def utils = call read('utils/userinfo.feature') {userSession: #(userA_SID)}
      * def userA = utils.userInfo
      # and different login methods should give the exact same user info but the session length
      * remove userA.sessionLength

    # Trying to register again with a session is an error,
    Given path '/auth/token/login'
      * params redirects
      * configure cookies = { sid: #(userA_SID) }
      * method get
    Then status 200
      * match responseCookies.tid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.eid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.sid contains {path: "/", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #(userA_SID)}
      * match response contains 'http://error.com'
      * match response !contains 'http://redirect.com'
      * match response !contains 'http://login.com'
    
    # but user shall not be changed.
    Given def utils = call read('utils/userinfo.feature') {userSession: #(userA_SID)}
      * match utils.userInfo contains userA

    # Login with the token shall be a success
    Given path '/auth/token/login'
      * params redirects
      * configure cookies = { tid: #(userA_TID) }
      * method get
    Then status 200
      * match responseCookies.tid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #(userA_TID)}
      * match responseCookies.eid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.sid contains {path: "/", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true}
      * match response contains 'http://redirect.com'
      * match response !contains 'http://error.com'
      * match response !contains 'http://login.com'
    # but session shall have been updated
    And match responseCookies.sid.value != userA_SID
      * def userA_SID = responseCookies.sid.value

    # while user shall be the same
    Given def utils = call read('utils/userinfo.feature') {userSession: #(userA_SID)}
      * match utils.userInfo contains userA

    # Login with the token shall be a success when rememberMe is set,
    Given path '/auth/token/login'
      * params redirects
      * params rememberMe = true
      * configure cookies = { tid: #(userA_TID) }
      * method get
    Then status 200
      # no new token should be generated
      * match responseCookies.tid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #(userA_TID)}
      * match responseCookies.eid contains {path: "/identity/auth", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, "max-age":#? _ < 0}
      * match responseCookies.sid contains {path: "/", domain:#(karate.properties['serviceDomain']), httponly:true, secure: true}
      * match response contains 'http://redirect.com'
      * match response !contains 'http://error.com'
      * match response !contains 'http://login.com'
    # but session shall have been changed
    And match responseCookies.sid.value != userA_SID
    * def userA_SID = responseCookies.sid.value

    # while user shall be the same
    Given def utils = call read('utils/userinfo.feature') {userSession: #(userA_SID)}
    * match utils.userInfo contains userA