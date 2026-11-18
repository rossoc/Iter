| baseline | rc=0 | clean 142.7s | incr 7.0s | 25.6 MB (26917392) | stripped 18.5 MB (19400928) | args:  |
| strip | rc=0 | clean 168.8s | incr 4.2s | 18.5 MB (19400928) | stripped 18.5 MB (19400992) | args: --config profile.release.strip=true |
| lto-thin | rc=0 | clean 123.1s | incr 32.4s | 26.0 MB (27328672) | stripped 18.9 MB (19864880) | args: --config profile.release.lto="thin" |
| lto-fat-cgu1 | rc=0 | clean 270.9s | incr 254.7s | 18.7 MB (19700416) | stripped 15.8 MB (16631232) | args: --config profile.release.lto="fat" --config profile.release.codegen-units=1 |
| opt-s | rc=0 | clean 103.3s | incr 5.1s | 26.8 MB (28136384) | stripped 17.2 MB (18069024) | args: --config profile.release.opt-level="s" |
| opt-z | rc=0 | clean 100.6s | incr 4.6s | 25.6 MB (26847136) | stripped 13.5 MB (14212368) | args: --config profile.release.opt-level="z" |
| no-default-features | rc=0 | clean 71.1s | incr 1.7s | 12.8 MB (13442976) | stripped 10.3 MB (10828784) | args: --no-default-features |
| release-noincr | rc=0 | clean 119.2s | incr 61.6s | 25.5 MB (26803968) | stripped 18.6 MB (19548752) | args: --config profile.release.incremental=false |
| dist-profile | rc=1 | clean 0.0s | incr 0.0s | 0.0 MB (0) | stripped 0.0 MB (0) | args: --profile dist |
| dist-opt-s | rc=1 | clean 0.0s | incr 0.0s | 0.0 MB (0) | stripped 0.0 MB (0) | args: --profile dist --config profile.dist.opt-level="s" |
| trim-sys-sqlite | rc=0 | clean 105.1s | incr 4.3s | 23.9 MB (25137952) | stripped 16.8 MB (17706544) | args:  |
| trim-native-tls | rc=101 | clean 0.1s | incr 0.1s | 0.0 MB (0) | stripped 0.0 MB (0) | args:  |
| dist+trims | rc=1 | clean 0.0s | incr 0.0s | 0.0 MB (0) | stripped 0.0 MB (0) | args: --profile dist |
| dev-baseline | rc=0/0 | dev clean 53.7s | dev incr 4.3s | test --no-run (after build) 17.6s | test --no-run incr 3.3s | debug bin 93 MB | target 2.2G | args:  |
| dev-ltonly-depsnodebug | rc=0/0 | dev clean 45.2s | dev incr 5.9s | test --no-run (after build) 15.7s | test --no-run incr 3.4s | debug bin 84 MB | target 1.7G | args: --config profile.dev.debug="line-tables-only" --config profile.dev.package."*".debug=false |
| dev-ltonly-depsnodebug-opt1 | rc=0/0 | dev clean 92.8s | dev incr 4.1s | test --no-run (after build) 15.9s | test --no-run incr 3.1s | debug bin 63 MB | target 1.6G | args: --config profile.dev.debug="line-tables-only" --config profile.dev.package."*".debug=false --config profile.dev.package."*".opt-level=1 |
| dist | rc=0 | clean 266.5s | incr 228.2s | 15.8 MB (16647504) | stripped 15.8 MB (16647552) | args: --config profile.release.lto="fat" --config profile.release.codegen-units=1 --config profile.release.strip=true --config profile.release.incremental=false |
| dist-opt-s | rc=0 | clean 213.5s | incr 179.2s | 12.1 MB (12783008) | stripped 12.1 MB (12783072) | args: --config profile.release.lto="fat" --config profile.release.codegen-units=1 --config profile.release.strip=true --config profile.release.incremental=false --config profile.release.opt-level="s" |
| trim-native-tls | rc=0 | clean 121.2s | incr 4.4s | 24.4 MB (25617872) | stripped 17.4 MB (18346688) | args:  |
| dist+sys-sqlite+native-tls | rc=0 | clean 264.1s | incr 211.0s | 13.4 MB (14128832) | stripped 13.4 MB (14128880) | args: --config profile.release.lto="fat" --config profile.release.codegen-units=1 --config profile.release.strip=true --config profile.release.incremental=false |
