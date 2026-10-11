using System;
using System.Collections.Generic;
using System.Web.Script.Serialization;
// Compiled with the unchanged recovered User.cs DTO. No original assembly or SQL execution.
class UserOracle {
    static void Main() {
        var fixtures = new List<object>();
        foreach(var id in new[] { int.MinValue, -1, 0, 1, int.MaxValue })
        foreach(var channel in new byte[] { 0, 1, 6, 255 }) {
            string name = "Estudo-" + id;
            var user = new LoginServiceCore.User {
                ID = id, Name = name, ServiceID = 42, ChannelingCode = null
            };
            if(channel != 0) user.ChannelingCode = channel;
            fixtures.Add(new {
                id = id, name = name, service_id = 42, channel_code = channel,
                next_character_sn = user.NextCharacterSN, channeling_code = user.ChannelingCode,
                last_login_ip = user.LastLoginIP, ip_lock_time = user.IPLockTime,
                second_password = user.SecondPassword, second_password_fail_count = user.SecondPasswordFailCount,
                second_password_last_fail = user.SecondPasswordLastFail, facebook_token = user.FacebookToken,
            });
        }
        Console.WriteLine(new JavaScriptSerializer().Serialize(new {
            runtime = Environment.Version.ToString(),
            method = "Unchanged recovered User.cs + new-user initializer copied from UserLoginProcessor; original services/database never executed",
            cases = fixtures,
        }));
    }
}