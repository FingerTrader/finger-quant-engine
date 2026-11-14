#!/usr/bin/env python3
"""Build/sign/run a disposable first-party App Sandbox probe; no Store credentials/Desk changes."""
import argparse, hashlib, json, os, plistlib, shutil, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def run(args):
    r=subprocess.run(list(map(str,args)),capture_output=True,timeout=45)
    if r.returncode:raise RuntimeError(f'{args[0]} exit {r.returncode}: '+r.stderr.decode(errors='replace')[-2500:])
    return r

def main():
    p=argparse.ArgumentParser();p.add_argument('--artifacts',type=Path,required=True);p.add_argument('--request',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    if sys.platform!='darwin':raise RuntimeError('macOS probe only; no cross-platform acceptance')
    out=a.output.resolve();out.mkdir(parents=True,exist_ok=False)
    bundle=out/'FqeBoundaryProbe.app';contents=bundle/'Contents';mac=contents/'MacOS';frameworks=contents/'Frameworks';resources=contents/'Resources'
    for d in [mac,frameworks,resources]:d.mkdir(parents=True)
    request=a.request.read_bytes();assert 0<len(request)<=262144
    (resources/'request.json').write_bytes(request)
    shutil.copy2(a.artifacts/'fqe-desktop-peer',mac/'fqe-desktop-peer');shutil.copy2(a.artifacts/'libfqe_bridge.dylib',frameworks/'libfqe_bridge.dylib')
    info=dict(CFBundleIdentifier='com.finger.fqe.boundaryprobe',CFBundleExecutable='FqeBoundaryProbe',CFBundlePackageType='APPL',CFBundleVersion='1',CFBundleShortVersionString='0.1.0')
    (contents/'Info.plist').write_bytes(plistlib.dumps(info))
    # Match actual Desk release sandbox/file/network keys, not debug's JIT/server grants.
    host={'com.apple.security.app-sandbox':True,'com.apple.security.files.user-selected.read-only':True,'com.apple.security.network.client':True}
    child={'com.apple.security.app-sandbox':True,'com.apple.security.inherit':True}
    for name,values in [('host',host),('child',child)]: (out/(name+'.entitlements')).write_bytes(plistlib.dumps(values))
    run(['/usr/bin/clang','-Wall','-Wextra','-Werror','-O2',ROOT/'examples/desktop/macos/probe.c','-framework','CoreFoundation','-framework','Security','-o',mac/'FqeBoundaryProbe'])
    run(['/usr/bin/install_name_tool','-id','@rpath/libfqe_bridge.dylib',frameworks/'libfqe_bridge.dylib'])
    run(['/usr/bin/codesign','--force','--sign','-',frameworks/'libfqe_bridge.dylib'])
    run(['/usr/bin/codesign','--force','--sign','-','--entitlements',out/'child.entitlements',mac/'fqe-desktop-peer'])
    run(['/usr/bin/codesign','--force','--sign','-','--entitlements',out/'host.entitlements',bundle])
    run(['/usr/bin/codesign','--verify','--strict','--deep',bundle])
    links={name:run(['/usr/bin/otool','-L',file]).stdout.decode() for name,file in [('peer',mac/'fqe-desktop-peer'),('ffi',frameworks/'libfqe_bridge.dylib')]}
    for value in links.values():
        for line in value.splitlines()[1:]:assert line.strip().startswith(('/usr/lib/','/System/Library/','@rpath/','@loader_path/')),'development native dependency'
    # Bundled resources/natives only; no Cargo checkout, compiler or Dart in child environment.
    reports={}
    for candidate in ['ffi','ipc']:
        r=run([mac/'FqeBoundaryProbe',candidate,ROOT/'README.md'])
        (out/(candidate+'-response.json')).write_bytes(r.stdout)
        value=json.loads(r.stdout);assert value['ok'] is True
        assert value['body']['report']['metrics']['final_equity']=='97.00'
        assert b'sandbox_entitled=1 outside_public_file_blocked=1' in r.stderr
        reports[candidate]=dict(exit=r.returncode,sandbox=r.stderr.decode().strip(),response_sha256=hashlib.sha256(r.stdout).hexdigest())
    assert (out/'ffi-response.json').read_bytes()==(out/'ipc-response.json').read_bytes()
    report=dict(format='fqe.desktop-sandbox-bundle-probe/1',status='passed',adhoc_only=True,actual_desk_app=False,installer_or_store_acceptance=False,candidates=reports,native_dependencies=links,host_entitlements=host,helper_entitlements=child)
    (out/'report.json').write_text(json.dumps(report,sort_keys=True,indent=2));print(json.dumps(report,sort_keys=True))
if __name__=='__main__':main()
