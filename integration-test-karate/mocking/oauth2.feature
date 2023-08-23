    @ignore
Feature: Oauth2 mocker server

  Background:
    * def utils = call read("../utils/utils.js")
    * print utils

  Scenario: pathMatches('/oauth2/token') && methodIs('post')
    * def process =
      """
        function() {
          const code = paramValue('code');
          const grant_type = paramValue('grant_type');
          const redirect_uri = paramValue('redirect_uri');
          const code_verifier = paramValue('code_verifier');
          const user = utils.parseQueryParams(code);

          if (!user || !user.id || !grant_type || !redirect_uri || !code_verifier) {
            return [400, null];
          }

          const response = {
            access_token: code, 
            token_type: 'Bearer'
          }
          return [200, response];
        }
      """
    * def result = process()
    * print result
    * def responseStatus = result[0]
    * def response = result[1]

  Scenario: pathMatches('/oauth2/users') && methodIs('get')
    * def process =
      """
        function() {
          const code = karate.request.header('authorization').split(' ')[1] ?? '';
          const user = parseCode(code);

          if (!user || !user.id) {
            return [400, null];
          }
          return [200, user];
        }
      """
    * def result = process()
    * print result
    * def responseStatus = result[0]
    * def response = result[1]

