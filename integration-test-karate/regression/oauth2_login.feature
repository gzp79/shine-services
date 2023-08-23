Feature: Oauth2 flow

  Background:
    * def utils = call read("../utils/utils.js")
    * url utils.identityUrl
    * def port = 8090
    * def fallbackErrorUrl = 'http://web.scytta-test.com:8080/error'
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
    Given def mock = karate.start({mock:'../mocking/oauth2.feature', port: port})

    Given path '/auth/oauth2_flow/auth'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=invalidInput&status=400'
      * match responseCookies contains deep utils.matchClearAuthCookies

  Scenario: Auth (parameters: VALID, cookie: NO) should be an error
    Given def mock = karate.start({mock:'../mocking/oauth2.feature', port: port})
    
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)
    Given path '/auth/oauth2_flow/auth'
      * params utils.defaultRedirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=authError&status=400'
      * match responseCookies contains deep utils.matchClearAuthCookies

  Scenario: Auth (parameters: NO, cookie: VALID) should be an error 
    Given def mock = karate.start({mock:'../mocking/oauth2.feature', port: port})
    
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * params utils.defaultRedirects
      * configure cookies = { eid: #(userEID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=invalidInput&status=400'
      * match responseCookies contains deep utils.matchClearAuthCookies

  Scenario: Auth (parameters: INVALID state, cookie: VALID) should be an error 
    Given def mock = karate.start({mock:'../mocking/oauth2.feature', port: port})
    
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * param rememberMe = true
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * params utils.defaultRedirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = "invalid"
      * configure cookies = null
      * cookie eid = userEID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.errorUrl + '?type=authError&status=400'
      * match responseCookies contains deep utils.matchClearAuthCookies

  Scenario: Auth (parameters: INVALID code, cookie: VALID) should be an error 
    Given def mock = karate.start({mock:'../mocking/oauth2.feature', port: port})

    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * params utils.defaultRedirects
      * param code = "invalid"
      * param state = authParams.state
      * configure cookies = { eid: #(userEID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.errorUrl + '?type=authError&status=500'
      * match responseCookies contains deep utils.matchClearAuthCookies
      * match response contains 'Server returned empty error response'

  Scenario: Auth (unreachable 3rd party) should be an error 
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * params utils.defaultRedirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = authParams.state
      * configure cookies = { eid: #(userEID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.errorUrl + '?type=authError&status=500'
      * match responseCookies contains deep utils.matchClearAuthCookies
      * match response contains 'No connection could be made because the target machine actively refused it.'

  Scenario: Login should register a new user
    Given def mock = karate.start({mock:'../mocking/oauth2.feature', port: port})
    
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * def randomName = utils.getRandomString(5);   
      * def userSUB = (utils.uuid())
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = { eid: #(userEID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userA_SID = responseCookies.sid.value

    Given def userA = (karate.call('../utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      * match userA contains {name: #(randomName)}
      * remove userA.sessionLength

    # login again
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * def randomName = utils.getRandomString(5);      
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = { eid: #(userEID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userB_SID = responseCookies.sid.value
    Given def userB = (karate.call('../utils/userinfo.feature', {userSession: userB_SID}).userInfo)
      * match userB contains userA

   Scenario: Login with remember me should register a new user
    Given def mock = karate.start({mock:'../mocking/oauth2.feature', port: port})
    
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * param rememberMe = true
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * def randomName = utils.getRandomString(5);   
      * def userSUB = (utils.uuid())
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = { eid: #(userEID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ > 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userA_SID = responseCookies.sid.value
      * def userA_TID = responseCookies.tid.value

    Given def userInfo = call read('../utils/userinfo.feature') {userSession: #(userA_SID)}
      * def userA = userInfo.userInfo
      * remove userA.sessionLength

    # Login with session should be an error
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = { sid: #(userA_SID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {value: #(userA_SID), "max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}

    # Login with token should be fine, but old tid cookie shall be cleared
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = { tid: #(userA_TID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}

    # Login again gives the same user
    Given path '/auth/oauth2_flow/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)}
    And def userEID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)

    Given path '/auth/oauth2_flow/auth'
      * param code = utils.createUrlQueryString({id: userSUB, name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = { eid: #(userEID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
      * def userB_SID = responseCookies.sid.value
    # and user shall be the same
    Given def userInfo = call read('../utils/userinfo.feature') {userSession: #(userB_SID)}
      * match userInfo.userInfo contains userA

    # Login with the token shall be a success and give the same user
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * configure cookies = { tid: #(userA_TID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {value: #(userA_TID), "max-age": #? _ > 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
    # but session shall have been changed
    And match responseCookies.sid.value != userA_SID
      * def userB_SID = responseCookies.sid.value
    # and user shall be the same
    Given def userInfo = call read('../utils/userinfo.feature') {userSession: #(userB_SID)}
      * match userInfo.userInfo contains userA
