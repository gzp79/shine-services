Feature: Check auth cookie validation

  Background:
    * def utils = karate.properties['utils']  
    * url utils.identityUrl
    
    # todo 
    # expired tid -> tid is removed
    # invalid signature sid -> sid is removed
    # invalid signature tid -> tid is removed
    # invalid signature eid -> eid is removed
    
  Scenario Outline: tid: ${tid}, sid: ${sid}, eid: ${eid}
    * def data = (karate.callonce('@create_all_cookies').data)    
    * def tid = (tid == '-' ? null : tid == '+' ? data['t'] : tid == '!' ? data['t2'] : karate.abort())
    * def sid = (sid == '-' ? null : sid == '+' ? data['s'] : sid == '!' ? data['s2'] : karate.abort())
    * def eid = (eid == '-' ? null : eid == '+' ? data['e'] : eid == '!' ? data['e2'] : karate.abort())
    * def requiredCookies = 
        """
        function() {
            const r = {
              tid: {'max-age': '#? _ < 0'},
              sid: {'max-age': '#? _ < 0'},
              eid: {'max-age': '#? _ < 0'},
            };
            const tokens = (required ?? '').split(',');
            for(const p of [['tid', ['t', 't2']], ['sid', ['s', 's2']], ['eid', ['e', 'e2']]]) {
              for(const c of p[1]) {
                let rc = r[p[0]]
                if (tokens.includes(c)) {
                  switch ( p[0] ) {
                    case 'tid': rc['max-age'] = '#? _ > 0'; break;
                    case 'sid': rc['max-age'] = utils.SESSION_SCOPE; break;
                    case 'eid': rc['max-age'] = utils.SESSION_SCOPE; break;
                  }
                  rc.value = data[c];
                }
              }
            }
            return r;
          }
        """
       * print requiredCookies()
     Given path '/auth/validate'
       * configure cookies = null
       * cookie tid = tid
       * cookie sid = sid
       * cookie eid = eid
       * method get
       * status 200
       * match responseCookies contains deep utils.matchAuthCookiesValidate
       * match responseCookies contains deep requiredCookies()

    Examples:
      # -: missing
      # !: not matching (ex different user id). When multiple ! are present in a row it's assumed all of them are different
      # +: ok, valid cookie
      | tid | sid | eid | required |
      |  -  |  -  |  -  |          |
      |  -  |  -  |  +  |          |
      |  -  |  -  |  !  |          |
      |  -  |  +  |  -  |  s       |
      |  -  |  +  |  +  |  s,e     |
      |  -  |  +  |  !  |  s       |
      |  -  |  !  |  -  |  s2      |
      |  -  |  !  |  +  |  s2      |
      |  -  |  !  |  !  |  s2      |

      |  +  |  -  |  -  |  t       |
      |  +  |  -  |  +  |  t       |
      |  +  |  -  |  !  |  t       |
      |  +  |  !  |  -  |  t       |
      |  +  |  !  |  +  |  t       |
      |  +  |  !  |  !  |  t       |
      |  +  |  +  |  -  |  t,s     |
      |  +  |  +  |  +  |  t,s,e   |
      |  +  |  +  |  !  |  t,s     |

      |  !  |  -  |  -  |  t2      |
      |  !  |  -  |  +  |  t2      |
      |  !  |  -  |  !  |  t2      |
      |  !  |  !  |  -  |  t2      |
      |  !  |  !  |  +  |  t2      |
      |  !  |  !  |  !  |  t2      |
      |  !  |  +  |  -  |  t2      |
      |  !  |  +  |  +  |  t2      |
      |  !  |  +  |  !  |  t2      |
      
    @ignore @create_all_cookies
  Scenario: Create all the required cookies
    * def u0 = (karate.call('@create_cookie').user)
    * def u1 = (karate.call('@create_cookie').user)
    * def u2 = (karate.call('@create_cookie').user)
    * def u3 = (karate.call('@create_cookie').user)
    * def data = 
    """
      ({
        t: u0.cookies.tid, s:u0.cookies.sid, e:u0.cookies.eid, 
        t2: u1.cookies.tid, s2:u2.cookies.sid, e2:u3.cookies.eid
      })
    """

    @ignore @create_cookie
  Scenario: Create a consistent cookie set 
    Given def user = (karate.call('../utils/create_user.feature').user)
      * path '/auth/oauth2_flow/link'
      * params utils.defaultRedirects
      * param rememberMe = true
      * configure cookies = user.cookies
      * method get
      * status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)} 
      * user['cookies']['eid'] = responseCookies.eid.value


        
