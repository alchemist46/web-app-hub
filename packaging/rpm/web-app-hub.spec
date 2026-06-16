%global appid org.pvermeer.WebAppHub

# We ship a pre-built release binary, so skip debuginfo extraction.
%global debug_package %{nil}

Name:           web-app-hub
Version:        %{appver}
Release:        1%{?dist}
Summary:        Create web apps with ease

License:        GPL-3.0-only
URL:            https://github.com/pvermeer/web-app-hub
Source0:        %{name}-%{version}.tar.gz

Requires:       gtk4
Requires:       libadwaita

%description
A modern Web App Manager built with Rust, GTK4 and the Adwaita design
language. Web App Hub manages web applications, each with its own icon and
isolated browser profile, reusing your existing browser installations.

%prep
%autosetup

%install
install -D -m0755 web-app-hub %{buildroot}%{_bindir}/web-app-hub
install -D -m0644 %{appid}.desktop \
    %{buildroot}%{_datadir}/applications/%{appid}.desktop
install -D -m0644 %{appid}.metainfo.xml \
    %{buildroot}%{_datadir}/metainfo/%{appid}.metainfo.xml
install -D -m0644 %{appid}.png \
    %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/%{appid}.png

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/%{appid}.desktop || :

%files
%license LICENSE
%{_bindir}/web-app-hub
%{_datadir}/applications/%{appid}.desktop
%{_datadir}/metainfo/%{appid}.metainfo.xml
%{_datadir}/icons/hicolor/256x256/apps/%{appid}.png

%changelog
* Tue Jun 16 2026 PVermeer <noreply@github.com> - %{appver}-1
- Local RPM build of Web App Hub
