"""Packaging comparisons against independent pre-change git object fixtures.

Fixtures are verbatim da81e38705 objects, not outputs of the staging function.
"""
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('stage', ROOT / 'tools/stage_launcher_packaging.py')
STAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STAGE)
FIXTURES = ROOT / 'tests/fixtures/launcher-upstream'

class PackagingIdentity(unittest.TestCase):
    @unittest.skipUnless(shutil.which("rustc"), "requires Rust for real compiled identity metadata")
    def test_compiled_namespace_metadata_preserves_upstream_and_isolates_fork(self):
        expected = json.loads((FIXTURES / 'identity.json').read_text())
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            # Compile the real identity module, not a Python model of its selection.
            program = '#[allow(dead_code)] #[path = ' + json.dumps(str(ROOT / 'launcher/src/identity.rs')) + '] mod identity;\nfn main() {\n'
            program += ''.join('println!("{}\\t{}", "' + name + '", identity::' + name + ');\n' for name in expected)
            program += '}\n'
            source = temporary / 'metadata.rs'
            source.write_text(program)
            outputs = []
            addresses = temporary / 'libsunrise_launcher_addresses.rlib'
            subprocess.run(['rustc', '--edition=2024', '--crate-name=sunrise_launcher_addresses', '--crate-type=rlib', str(ROOT / 'launcher/crates/addresses/src/lib.rs'), '-o', str(addresses)], check=True)
            build_script = temporary / 'build-script'
            subprocess.run(['rustc', '--edition=2021', str(ROOT / 'launcher/build.rs'), '-o', str(build_script)], check=True)
            for repository in (STAGE.UPSTREAM, STAGE.FORK):
                env = dict(os.environ, SUNRISE_LAUNCHER_REPOSITORY=repository,
                           CARGO_CFG_TARGET_OS='linux', CARGO_CFG_TARGET_ENV='gnu',
                           CARGO_MANIFEST_DIR=str(ROOT / 'launcher'))
                directives = subprocess.check_output([str(build_script)], env=env, text=True)
                flags = []
                for line in directives.splitlines():
                    if line.startswith('cargo:rustc-cfg='):
                        flags += ['--cfg', line.removeprefix('cargo:rustc-cfg=')]
                binary = temporary / 'metadata'
                subprocess.run(['rustc', '--edition=2021', str(source), '-o', str(binary), '--extern', f'sunrise_launcher_addresses={addresses}', *flags], check=True)
                output = subprocess.check_output([str(binary)], text=True)
                outputs.append(dict(line.split('\t', 1) for line in output.splitlines()))
            upstream, fork = outputs
            self.assertEqual(upstream, expected)
            # A Sunrise build has its own name, data folders, saved accounts and install: none of it
            # takes a Sunrise launcher's place (it only opens Sunrise's folders while its own are missing).
            self.assertTrue(all(fork[name] != upstream[name] for name in expected))
            self.assertEqual(fork['WINDOW_TITLE'], 'Sunrise Launcher')
            self.assertEqual(fork['APP_ID'], 'net.playsunrise.launcher')
            self.assertEqual(fork['LAUNCHER_FOLDER'], 'sunrise/launcher')
            self.assertEqual(fork['EXECUTABLE'], 'sunrise-launcher')
            self.assertEqual(fork['INSTALL_KEY'], 'Software\\Sunrise\\Launcher')

    def test_real_workflow_repository_input_and_matrix(self):
        workflow = (ROOT / '.github/workflows/launcher.yml').read_text()
        self.assertIn('SUNRISE_LAUNCHER_REPOSITORY: ${{ github.repository }}', workflow)
        self.assertIn('--repository "$GITHUB_REPOSITORY" --tag', workflow)
        original = (ROOT / 'launcher/tests/fixtures-launcher-workflow-upstream.yml').read_text()
        block = original.split('      matrix:\n', 1)[1].split('    runs-on:', 1)[0]
        expected = []
        for line in block.splitlines():
            if line.startswith('          - name: '):
                expected.append({'name': line.split(': ', 1)[1]})
            elif line.startswith('            ') and not line.strip().startswith('#') and ': ' in line:
                key, value = line.strip().split(': ', 1)
                expected[-1][key] = True if value == 'true' else value
        for repository in (STAGE.UPSTREAM, STAGE.FORK):
            output = subprocess.check_output(['python3', str(ROOT / 'tools/launcher_build_matrix.py')],
                                            env=dict(os.environ, GITHUB_REPOSITORY=repository), text=True)
            entries = json.loads(output)['include']
            if repository == STAGE.UPSTREAM:
                self.assertEqual(entries, expected)
            else:
                self.assertEqual([entry['name'] for entry in entries], ['linux', 'macos', 'windows'])
                self.assertEqual([entry['asset'] for entry in entries],
                                 ['sunrise-launcher-linux-x86_64.tar.gz', 'sunrise-launcher-macos-arm64.zip', 'sunrise-launcher-windows-x64.zip'])
                self.assertEqual(entries[1], json.loads(
                    (ROOT / 'tests/fixtures/launcher-fork-macos.json').read_text()))
                self.assertEqual(entries[2]['setup_asset'], 'sunrise-launcher-setup.exe')

    def test_upstream_metadata_is_byte_identical_to_base_objects(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / 'packaging'
            STAGE.stage(ROOT / 'packaging', destination, STAGE.UPSTREAM)
            # Installation identity stays fixed; packaging scripts evolve independently
            # (for example to ship the notices required by the current licence).
            for fixture in FIXTURES.glob('packaging_*'):
                if fixture.suffix == '.sh':
                    continue
                relative = fixture.name.removeprefix('packaging_').replace('_', '/')
                self.assertEqual((destination / relative).read_bytes(), fixture.read_bytes(), relative)
            for name, digest in json.loads((FIXTURES / 'icon-sha256.json').read_text()).items():
                self.assertEqual(hashlib.sha256((destination / name.removeprefix('packaging/')).read_bytes()).hexdigest(), digest, name)

    def test_fork_staging_does_not_modify_source_and_has_distinct_bundle(self):
        source = ROOT / 'packaging'
        before = {p.relative_to(source): p.read_bytes() for p in source.rglob('*') if p.is_file()}
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / 'packaging'
            STAGE.stage(source, destination, STAGE.FORK)
            plist = (destination / 'macos/Info.plist').read_text()
            self.assertIn('<string>net.playsunrise.launcher</string>', plist)
            self.assertIn('<string>sunrise-launcher</string>', plist)
            self.assertNotIn('<string>Sunrise</string>', plist)
            self.assertEqual((destination / 'icons/sunrise.icns').read_bytes(),
                             (source / 'icons/fork/launcher.icns').read_bytes())
            script = (destination / 'macos/package.sh').read_text()
            self.assertIn('identity=-', script)
            self.assertIn('zip=sunrise-launcher-macos-$arch.zip', script)
            self.assertIn('dmg=$out/sunrise-launcher-macos-$arch.dmg', script)
            self.assertIn('\"Sunrise Launcher\" \"$dmg\"', script)
            desktop = (destination / 'linux/net.playsunrise.launcher.desktop').read_text()
            self.assertIn('Name=Sunrise Launcher\n', desktop)
            self.assertIn('Exec=sunrise-launcher\n', desktop)
            self.assertIn('StartupWMClass=net.playsunrise.launcher\n', desktop)
            for script in ('linux/appimage.sh', 'macos/package.sh'):
                subprocess.run(['bash', '-n', str(destination / script)], check=True)
            self.assertNotEqual((destination / 'icons/sunrise.ico').read_bytes(), (source / 'icons/sunrise.ico').read_bytes())
        self.assertEqual(before, {p.relative_to(source): p.read_bytes() for p in source.rglob('*') if p.is_file()})

if __name__ == '__main__':
    unittest.main()
