
    @ignore
Feature: Utility scenarios

  Scenario: Get current user info
    Given url karate.properties['utils'].identityUrl
      * path '/api/auth/user/info'
      * configure cookies = { sid: #(userSession) }
      * method get
    Then status 200
      * match response contains { name: '#notnull', userId: '#uuid'}
    And def userInfo = $
