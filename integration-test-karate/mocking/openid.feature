    @ignore
Feature: OpenId Connect mocker server

  Background:
    * def base = 'http://mock.localhost.com:8090'
    * def utils = call read("../utils/utils.js") 
    * def keySet = 
      """
      ({
        // new key set can be generated at https://mkjwk.org/ quite easily 
        // (RSA, size:512, Use:Signature, Alg:RS256, ID:Sha-1 )
        "keys": [
            {
                "p": "zPRZpSga1P7C1f7E22Ff1dtX0yRqAEYI-wiAs4ErOTs",
                "kty": "RSA",
                "q": "ppZYEAObgyx9KboeGzuN5WWHLAOQYx4za90P0u0fB_s",
                "d": "SPIrjMiYv9TDqlwt6ruXPSdN7_Gvcb7EFOmNAezma4ZFpCmFIAmUlmqsCHDr2G2G0S6-YrvQbiAd514fhMMLNQ",
                "e": "AQAB",
                "use": "sig",
                "kid": "CjQIpywjVwYF5RunzrkjlJpLvSE=",
                "qi": "yt1my8bfkWdWc4EU2wr1sa4FMbLB7aeKPhZ6mSHOy0Q",
                "dp": "ficNIrZLxbzGClgVrX8DMSwgo8rvIBn7nyC9rz-bbk8",
                "alg": "RS256",
                "dq": "X-KFiaIp7tS6rjvcfFxJDlLj_OeIQiTuABXbt9KYW-U",
                "n": "hV7Pzm_Ao_WZDQwM4Es0HpXYElZtKppKoVFWjGiH3qwixw7utaXohwDakjEPxrm4Er0ZLWhGKqev6yglYRa52Q"
            }
        ]
      })
      """

  Scenario: pathMatches('/openid/.well-known/openid-configuration') && methodIs('get')
    * def response = 
    """
    ({
        issuer: base + '/openid',
        jwks_uri: base + '/openid/jwks',
        authorization_endpoint: base + '/openid/authorize',
        token_endpoint: base + '/openid/token',
        userinfo_endpoint: base + '/openid/userinfo',
        response_types_supported: ['id_token'],
        subject_types_supported: ['public'],
        id_token_signing_alg_values_supported: ['RS256'],
    })
    """

  Scenario: pathMatches('/openid/jwks') && methodIs('get')
    * def response = keySet

  Scenario: pathMatches('/openid/token') && methodIs('post')
    * def process =
      """
        function() {
          const code = paramValue('code');
          const grant_type = paramValue('grant_type');
          const redirect_uri = paramValue('redirect_uri');
          const code_verifier = paramValue('code_verifier');
          const user = utils.getUrlQueryParams(code);

          if (!user || !user.id || !grant_type || !redirect_uri || !code_verifier) {
            return [400, null];
          }

          const response = {
            access_token: code, 
            //token_type: 'Bearer'
          }
          return [200, response];
        }
      """
    * def result = process()
    * print result
    * def responseStatus = result[0]
    * def response = result[1]


  Scenario: methodIs('get')
      * def responseStatus = 404
