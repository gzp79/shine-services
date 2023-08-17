Feature: Utils to create new users

    @ignore @create_guest
  Scenario: Create a new guest user
    * def utils = karate.properties['utils']    
    
    Given url utils.identityUrl
      * path '/auth/token/login'
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
    And def user = 
        """
        ({
          cookies: {
            sid: responseCookies.sid.value,
            tid: responseCookies.tid.value    
          },
          info: karate.call('../utils/userinfo.feature', {userSession: responseCookies.sid.value}).userInfo
        })
        """
      * print user
