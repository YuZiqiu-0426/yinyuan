import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { environment } from '../../environments/environment';
import { AuthSessionService } from '../core/auth-session.service';

@Component({
  selector: 'app-main-shell',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  templateUrl: './main-shell.component.html',
})
export class MainShellComponent {
  protected readonly auth = inject(AuthSessionService);
  protected readonly devBypassAuth = environment.devBypassAuth;
  protected readonly navOpen = signal(false);

  protected toggleNav(): void {
    this.navOpen.update((v) => !v);
  }

  protected closeNav(): void {
    this.navOpen.set(false);
  }

  protected logout(): void {
    this.auth.logout();
  }
}
