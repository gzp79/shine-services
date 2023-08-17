Feature: Token credentials

  Background:
    * def utils = karate.properties['utils']    
    * url utils.identityUrl
    
  Scenario: Login without a token should redirect user to the login page
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.loginUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate

  Scenario: Login without invalid input should redirect to the default error page
    Given path '/auth/token/login'
      * param rememberMe = "invalid value"
      * configure cookies = null
      * method get
    Then status 200
      * def redirectUrl = utils.getRedirectUrl(response)
      * def redirectParams = utils.getUrlQueryParams(redirectUrl)
      * match redirectParams contains {type:"invalidInput", status: "400"}

  Scenario: Login without a token with explicit no-rememberMe should redirect user to the login page
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * param rememberMe = false
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.loginUrl
      * match responseCookies contains deep utils.matchClearAuthCookies
      
  Scenario: Login with 'rememberMe' should register a new user
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * param rememberMe = true
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ > 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
    And def userA_SID = responseCookies.sid.value
    # Waiting to check session length too
    Given eval java.lang.Thread.sleep(1000)
      * def userInfo = (karate.call('../utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      * match userInfo contains {name: #? _.startsWith('Freshman_'), sessionLength: #? _ >= 1}

  Scenario: Registering a new user should be able to log in
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * param rememberMe = true
      * configure cookies = null
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ > 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
    And def userA_SID = responseCookies.sid.value
      * def userA_TID = responseCookies.tid.value    
    # Getting user info shall be success
    Given def userA = (karate.call('../utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      # and different login methods should give the exact same user info but the session length
      * remove userA.sessionLength

    # Trying to register again with a session is an error,
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * configure cookies = { sid: #(userA_SID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {"max-age": #? _ < 0}
      * match responseCookies.sid contains {value: #(userA_SID), "max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}   
    # but user shall not be changed.
    Given def userInfo = (karate.call('../utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      * match userInfo contains userA

    # Trying to register again with a session and a token is an error,
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * configure cookies = { sid: #(userA_SID), tid: #(userA_TID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {value: #(userA_TID), "max-age": #? _ > 0}
      * match responseCookies.sid contains {value: #(userA_SID), "max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0} 
    # but user shall not be changed.
    Given def userInfo = (karate.call('../utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      * match userInfo contains userA

    # Login with the token shall be a success
    # For test with mismatching token and session please check the auth session test cases 
    # that ensures the consistency of the different cookies
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
    # but session shall have been updated
    And match responseCookies.sid.value != userA_SID
      * def userA_SID = responseCookies.sid.value
    # while user shall be the same
    Given def userInfo = (karate.call('../utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      * match userInfo contains userA

    # Login with the token shall be a success when rememberMe is set,
    Given path '/auth/token/login'
      * params utils.defaultRedirects
      * params rememberMe = true
      * configure cookies = { tid: #(userA_TID) }
      * method get
    Then status 200
      * match utils.getRedirectUrl(response) == utils.defaultRedirects.redirectUrl
      # no new token should be generated
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.tid contains {value: #(userA_TID), "max-age": #? _ > 0}
      * match responseCookies.sid contains {"max-age": #(utils.SESSION_SCOPE)}
      * match responseCookies.eid contains {"max-age": #? _ < 0}
    # but session shall have been changed
    And match responseCookies.sid.value != userA_SID
      * def userA_SID = responseCookies.sid.value
    # while user shall be the same
    Given def userInfo = (karate.call('../utils/userinfo.feature', {userSession: userA_SID}).userInfo)
      * match userInfo contains userA
