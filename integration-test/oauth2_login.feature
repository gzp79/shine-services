Feature: Oauth2 credentials

  Background:
    * url karate.properties['identityUrl']
    * callonce read('utils/login_defs.feature')
    * def utils = karate.properties['utils']      
    * def port = 8090
    * def fallbackErrorUrl = 'http://test.scytta.com:8080/scytta.com/error'
    * configure afterScenario = 
      """
        function() {
          if(mock) {
            console.log("stopping mock...");
            mock.stop();
          }
        }
      """

  Scenario: Auth (parameters: NO, cookie: NO) should be an error
    Given def mock = karate.start({mock:'mocking/oauth2.feature', port: port})

    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=invalidInput&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Auth (parameters: VALID, cookie: NO) should be an error
    Given def mock = karate.start({mock:'mocking/oauth2.feature', port: port})
    
    Given call read('@start_login') {rememberMe: false}
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=authError&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Auth (parameters: NO, cookie: VALID) should be an error 
    Given def mock = karate.start({mock:'mocking/oauth2.feature', port: port})
    
    Given call read('@start_login') {rememberMe: false}
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=invalidInput&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Auth (parameters: INVALID state, cookie: VALID) should be an error 
    Given def mock = karate.start({mock:'mocking/oauth2.feature', port: port})
    
    Given call read('@start_login') {rememberMe: true}
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = "invalid"
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '?type=authError&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Auth (parameters: INVALID code, cookie: VALID) should be an error 
    Given def mock = karate.start({mock:'mocking/oauth2.feature', port: port})

    Given call read('@start_login') {rememberMe: false}
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = "invalid"
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '?type=authError&status=500'
      * match responseCookies contains deep cookieNone

  Scenario: Auth (unreachable 3rd party) should be an error 
    Given call read('@start_login') {rememberMe: true}
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '?type=authError&status=500'
      * match responseCookies contains deep cookieNone

  Scenario: Login should register a new user
    Given def mock = karate.start({mock:'mocking/oauth2.feature', port: port})
    
    Given call read('@start_login') {rememberMe: false}
    Given path '/auth/oauth2_flow/auth'
      * def randomName = utils.getRandomString(5);   
      * def userSUB = (utils.uuid())
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.redirectUrl
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #(SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userA_SID = responseCookies.sid.value

    Given def userA = (karate.call('utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      * match userA contains {name: #(randomName)}
      * remove userA.sessionLength

    # login again
    Given call read('@start_login') {rememberMe: false}
    Given path '/auth/oauth2_flow/auth'
      * def randomName = utils.getRandomString(5);      
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.redirectUrl
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #(SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userB_SID = responseCookies.sid.value
    Given def userB = (karate.call('utils/userinfo.feature', {userSession: userB_SID}).userInfo)
      * match userB contains userA

   Scenario: Login with remember me should register a new user
    Given def mock = karate.start({mock:'mocking/oauth2.feature', port: port})
    
    Given call read('@start_login') {rememberMe: true}
     Given path '/auth/oauth2_flow/auth'
      * def randomName = utils.getRandomString(5);   
      * def userSUB = (utils.uuid())
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.redirectUrl
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ > 0}
      * match responseCookies.sid contains {"max-age": #(SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userA_SID = responseCookies.sid.value
      * def userA_TID = responseCookies.tid.value

    Given def userInfo = call read('utils/userinfo.feature') {userSession: #(userA_SID)}
      * def userA = userInfo.userInfo
      * remove userA.sessionLength

    # Login with session should be an error
    Given path '/auth/oauth2_flow/login'
      * params redirects
      * configure cookies = null
      * cookie sid = userA_SID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '?type=logoutRequired&status=400'
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {value: #(userA_SID), "max-age": #(SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}

    # Login with token should be fine, but old tid cookie shall be cleared
    Given path '/auth/oauth2_flow/login'
      * params redirects
      * configure cookies = null
      * cookie sid = userA_TID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://test.external-scytta.com:8090/oauth2/authorize'
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(SESSION_SCOPE)}

    # Login again gives the same user
    Given call read('@start_login') {rememberMe: false}
    Given path '/auth/oauth2_flow/auth'
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.redirectUrl
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #(SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userB_SID = responseCookies.sid.value
    # and user shall be the same
    Given def userInfo = call read('utils/userinfo.feature') {userSession: #(userB_SID)}
      * match userInfo.userInfo contains userA

    # Login with the token shall be a success and give the same user
    Given path '/auth/token/login'
      * params redirects
      * configure cookies = { tid: #(userA_TID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.redirectUrl
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {value: #(userA_TID), "max-age": #? _ > 0}
      * match responseCookies.sid contains {"max-age": #(SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
    # but session shall have been changed
    And match responseCookies.sid.value != userA_SID
      * def userB_SID = responseCookies.sid.value
    # and user shall be the same
    Given def userInfo = call read('utils/userinfo.feature') {userSession: #(userB_SID)}
      * match userInfo.userInfo contains userA

    @ignore @start_login
  Scenario: Start login flow
    Given path '/auth/oauth2_flow/login'
      * params redirects
      * param rememberMe = rememberMe
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://test.external-scytta.com:8090/oauth2/authorize'
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)
