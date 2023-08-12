Feature: Oauth2 credentials

  Background:
    * url karate.properties['identityUrl']
    * call read('utils/login_defs.feature')
    * def utils = karate.properties['utils']      
    * def port = 8090
    * def fallbackErrorUrl = 'http://test.scytta.com:8080/scytta.com/error'

  Scenario: Login without parameters or cookie should be an error
    Given call read('@start_login')
    Given karate.start({mock:'mocking/oauth2.feature', port: port})

    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=invalidInput&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Login without parameters should be an error
    Given call read('@start_login')
    Given karate.start({mock:'mocking/oauth2.feature', port: port})

    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * configure cookies = null
      * cookie eid = userA_EID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=invalidInput&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Login without cookie should be an error
    Given call read('@start_login')
    Given karate.start({mock:'mocking/oauth2.feature', port: port})
  
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == fallbackErrorUrl + '?type=authError&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Login with invalid state should be an error
    Given call read('@start_login')
    Given karate.start({mock:'mocking/oauth2.feature', port: port})
  
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = "invalid"
      * configure cookies = null
      * cookie eid = userA_EID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '/?type=authError&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Login with invalid code should be an error
    Given call read('@start_login')
    Given karate.start({mock:'mocking/oauth2.feature', port: port})
  
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = "invalid"
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userA_EID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '/?type=authError&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Login with unreachable 3rd party should be an error
    Given call read('@start_login')
  
    Given path '/auth/oauth2_flow/auth'
      * params redirects
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: 'n', email: 'n@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userA_EID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '?type=authError&status=400'
      * match responseCookies contains deep cookieNone

  Scenario: Login should register a new user
    Given call read('@start_login')
    Given karate.start({mock:'mocking/oauth2.feature', port: port})    

    Given path '/auth/oauth2_flow/auth'
      * def randomName = utils.getRandomString(5);
      * param code = utils.createUrlQueryString({id: utils.uuid(), name: randomName, email: randomName+'@a.com'})
      * param state = authParams.state
      * configure cookies = null
      * cookie eid = userA_EID
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == redirects.errorUrl + '?type=authError&status=400'
      * match responseCookies contains deep cookieNone

    # todo: userinfo is correct

    Scenario: Login with remember me should register a new user
      # todo: login with session is an error
      # todo: login with tid is an error ?
      # todo: login again gives the same user

    @ignore @start_login
  Scenario: Start login flow
    Given path '/auth/oauth2_flow/login'
      * params redirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) contains 'http://test.external-scytta.com:8090/oauth2/authorize'
      * match responseCookies contains deep cookieDefaults
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {"max-age": #? _ < 0}
      * match responseCookies.eid contains {"max-age": #(SESSION_SCOPE)}
    And def userA_EID = responseCookies.eid.value
      * def authUrl = utils.getRedirectUrl(response)
      * def authParams = utils.getUrlQueryParams(authUrl)
